
use std::sync::{Arc, RwLock};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use email::Email;
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
    Unknown = 255,
}
impl WorkerStatus {
    pub fn from_u8(value: u8) -> WorkerStatus {
        match value {
            0 => WorkerStatus::Ok,
            1 => WorkerStatus::Terminated,
            2 => WorkerStatus::ChannelDisconnected,
            3 => WorkerStatus::LockPoisoned,
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

    fn send_email(&mut self, email: Email) -> WorkerStatus
    {
        // For now, just display and forget (FIXME)
        println!("{:?}", email);

        WorkerStatus::Ok
    }
}

// Get MX records for a domain, in order of preference
fn get_mx_records_for_domain(domain: String) -> Result<Vec<String>, Error>
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
