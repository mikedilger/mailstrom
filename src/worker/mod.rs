
mod delivery;

use std::sync::{Arc, RwLock};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;
use std::net::SocketAddr;

use email::{Email, DeliveryResult};
use storage::MailstromStorage;
use error::Error;

pub enum Message {
    /// Ask the worker to deliver an email
    SendEmail(Email),
    /// Ask the worker to terminate
    Terminate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum WorkerStatus {
    Ok = 0,
    Terminated = 1,
    ChannelDisconnected = 2,
    LockPoisoned = 3,
    StorageWriteFailed = 4,
    Unknown = 255,
}
impl WorkerStatus {
    pub fn from_u8(value: u8) -> WorkerStatus {
        match value {
            0 => WorkerStatus::Ok,
            1 => WorkerStatus::Terminated,
            2 => WorkerStatus::ChannelDisconnected,
            3 => WorkerStatus::LockPoisoned,
            4 => WorkerStatus::StorageWriteFailed,
            _ => WorkerStatus::Unknown,
        }
    }
}

pub struct Worker<S: MailstromStorage + 'static>
{
    pub receiver: mpsc::Receiver<Message>,

    worker_status: Arc<AtomicU8>,

    helo_name: String,

    // Persistent shared storage
    storage: Arc<RwLock<S>>,
}

impl<S: MailstromStorage + 'static> Worker<S>
{
    pub fn new(receiver: mpsc::Receiver<Message>,
               storage: Arc<RwLock<S>>,
               worker_status: Arc<AtomicU8>,
               helo_name: &str)
               -> Worker<S>
    {
        Worker {
            receiver: receiver,
            worker_status: worker_status,
            helo_name: helo_name.to_owned(),
            storage: storage,
        }
    }

    pub fn run(&mut self) {

        let timeout: Duration = Duration::from_secs(60);

        loop {
            match self.receiver.recv_timeout(timeout) {
                Ok(message) => match message {
                    Message::SendEmail(email) => {
                        let worker_status = self.send_email(email);
                        if worker_status != WorkerStatus::Ok {
                            self.worker_status.store(worker_status as u8,
                                                     Ordering::SeqCst);
                        }
                        return;
                    }
                    Message::Terminate => {
                        println!("Terminating");
                        self.worker_status.store(
                            WorkerStatus::Terminated as u8, Ordering::SeqCst);
                        return;
                    },
                },
                Err(RecvTimeoutError::Timeout) => { },
                Err(RecvTimeoutError::Disconnected) => {
                    self.worker_status.store(WorkerStatus::ChannelDisconnected as u8,
                                             Ordering::SeqCst);
                    return;
                }
            };

        }
    }

    fn send_email(&mut self, mut email: Email) -> WorkerStatus
    {
        get_mx_records_for_email(&mut email);

        'next_recipient:
        for recipient in &mut email.recipients {

            // Skip if already completed
            match recipient.result {
                DeliveryResult::Delivered(_) => continue,
                DeliveryResult::Failed(_) => continue,
                _ => {}
            }

            let mut attempt: u8 = 0;
            let deferred_data: Option<(u8, String)> =
                if let DeliveryResult::Deferred(a, ref msg) = recipient.result {
                    Some((a, msg.clone()))
                } else {
                    None
                };
            if deferred_data.is_some() {
                let (a,msg) = deferred_data.unwrap();
                // Try again
                attempt = a + 1;
                if a == 3 {
                    // Or fail if too many attempts
                    recipient.result = DeliveryResult::Failed(
                        format!("Failed after 3 attempts: {}", msg));
                    continue;
                }
            }

            // Skip (and complete) if no MX servers
            if recipient.mx_servers.is_none() {
                recipient.result = DeliveryResult::Failed(
                    "MX records found but none are valid".to_owned());
                continue;
            }

            // Sequence through MX servers
            let mx_servers: &Vec<SocketAddr> = recipient.mx_servers.as_ref().unwrap();

            for i in recipient.current_mx .. mx_servers.len() {

                // Mark completed if this MX server is known to have been successfully
                // delivered to already
                if email.delivered_to_mx.contains(&mx_servers[i]) {
                    recipient.result = DeliveryResult::Delivered(
                        "[was delivered along with another recipients delivery]".to_owned());
                    continue 'next_recipient;
                }

                // Attempt delivery to this MX server
                recipient.result = ::worker::delivery::mx_delivery(
                    &email.rfc_email, email.message_id.clone(),
                    &mx_servers[i], &*self.helo_name, attempt);

                match recipient.result {
                    DeliveryResult::Delivered(_) => {
                        // save in delivered_to_mx list
                        email.delivered_to_mx.push(mx_servers[i].clone());
                        // Exit mx loop
                        break;
                    },
                    DeliveryResult::Deferred(_,_) => { } // continue MX loop
                    _ => {
                        // Exit mx loop
                        break;
                    }
                }
            }
        }

        // Lock the storage
        let mut guard = match (*self.storage).write() {
            Ok(guard) => guard,
            Err(e) => {
                println!("{:?}", e);
                return WorkerStatus::LockPoisoned;
            },
        };

        // Store the email delivery result (so far) into storage
        if let Err(e) = (*guard).store(&email) {
            println!("{:?}", e);
            return WorkerStatus::StorageWriteFailed;
        }

        WorkerStatus::Ok
    }
}

// Get MX records for email recipients
fn get_mx_records_for_email(email: &mut Email)
{
    use std::net::{SocketAddr, ToSocketAddrs};

    // Look-up the MX records for each recipient
    for recipient in &mut email.recipients {
        let mx_records = match get_mx_records_for_domain(&*recipient.domain) {
            Err(e) => {
                recipient.result = DeliveryResult::Failed(
                    format!("Unable to fetch MX record: {:?}", e));
                println!("MX LOOKUP FAILED FOR {}", recipient.email_addr);
                continue;
            }
            Ok(records) => {
                let mut mx_records: Vec<SocketAddr> = Vec::new();
                for record in records {
                    match (&*record, 25_u16).to_socket_addrs() {
                        Err(_) => {
                            println!("ToSocketAddr FAILED FOR {}: {}",
                                     recipient.email_addr,
                                     &*record);
                            continue; // MX record invalid?
                        },
                        Ok(mut iter) => match iter.next() {
                            Some(sa) => mx_records.push(sa),
                            None => continue, // No MX records
                        }
                    }
                }
                if mx_records.len() == 0 {
                    recipient.result = DeliveryResult::Failed(
                        "MX records found but none are valid".to_owned());
                    continue;
                }
                mx_records
            }
        };
        recipient.mx_servers = Some(mx_records);
        println!("DEBUG: got mx servers for {}: {:?}",
                 recipient.email_addr,
                 recipient.mx_servers.as_ref().unwrap());
    }
}

// Get MX records for a domain, in order of preference
fn get_mx_records_for_domain(domain: &str) -> Result<Vec<String>, Error>
{
    use resolv::{Resolver, Class, RecordType};
    use resolv::Record;
    use resolv::record::MX;

    let mut resolver = match Resolver::new() {
        Some(r) => r,
        None => return Err(Error::DnsUnavailable),
    };

    let mut response = try!(resolver.query(domain.as_bytes(),
                                           Class::IN,
                                           RecordType::MX));

    let mut records: Vec<Record<MX>> = response.answers::<MX>().collect();
    records.sort_by(|a,b| a.data.preference.cmp(&b.data.preference));
    Ok(records.into_iter().map(|rmx| rmx.data.exchange).collect())
}
