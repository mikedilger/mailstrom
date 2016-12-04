
mod delivery;
mod task;

use std::sync::{Arc, RwLock};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::atomic::{AtomicU8, Ordering};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use std::net::SocketAddr;

use internal_status::InternalStatus;
use status::DeliveryResult;
use storage::MailstromStorage;
use error::Error;
use self::task::{Task, TaskType};

pub enum Message {
    /// Ask the worker to deliver an email
    SendEmail(InternalStatus),
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

    // A list of tasks we need to do later, sorted in time order
    tasks: BTreeSet<Task>
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
            tasks: BTreeSet::new(),
        }
    }

    pub fn run(&mut self) {

        // This timeout represents how long we wait for a message.  If there are any
        // tasks in the tasklist, this will be the tiem until the first task is
        // due.  Otherwise it is set to 60 seconds.
        let mut timeout: Duration = Duration::from_secs(60);

        loop {
            debug!("(worker) waiting for a message ({} seconds)", timeout.as_secs());

            // Receive a message.  Waiting at most until the time when the next task
            // is due, or 60 seconds if there are no tasks
            match self.receiver.recv_timeout(timeout) {
                Ok(message) => match message {
                    Message::SendEmail(internal_status) => {
                        debug!("(worker) received SendEmail command");
                        let worker_status = self.send_email(internal_status);
                        if worker_status != WorkerStatus::Ok {
                            self.worker_status.store(worker_status as u8,
                                                     Ordering::SeqCst);
                            info!("(worker) failed and terminated");
                            return;
                        }
                    }
                    Message::Terminate => {
                        debug!("(worker) received Terminate command");
                        self.worker_status.store(
                            WorkerStatus::Terminated as u8, Ordering::SeqCst);
                        info!("(worker) terminated");
                        return;
                    },
                },
                Err(RecvTimeoutError::Timeout) => { },
                Err(RecvTimeoutError::Disconnected) => {
                    self.worker_status.store(WorkerStatus::ChannelDisconnected as u8,
                                             Ordering::SeqCst);
                    info!("(worker) failed and terminated");
                    return;
                }
            };

            // Copy out all the tasks that are due
            let now = Instant::now();
            let due_tasks: Vec<Task> = self.tasks.iter()
                .filter(|t| now > t.time).cloned().collect();

            // Handle all these due tasks
            for task in &due_tasks {
                let worker_status = self.handle_task(task);
                if worker_status != WorkerStatus::Ok {
                    self.worker_status.store(worker_status as u8,
                                             Ordering::SeqCst);
                    debug!("(worker) failed and terminated");
                    return;
                }
                self.tasks.remove(task);
            }

            // Recompute the timeout
            let now = Instant::now();
            timeout = if let Some(task) = self.tasks.iter().next() {
                debug!("(worker) looping (tasks in queue)");
                if task.time > now {
                    task.time - now
                } else {
                    Duration::new(0,0) // overdue!
                }
            } else {
                debug!("(worker) looping (no tasks)");
                Duration::from_secs(60)
            };
        }
    }

    fn send_email(&mut self, mut internal_status: InternalStatus) -> WorkerStatus
    {
        get_mx_records_for_email(&mut internal_status);

        let mut some_deferred_with_attempts: Option<u8> = None;

        'next_recipient:
        for recipient in &mut internal_status.recipients {

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
                    debug!("(worker) delivery failed after 3 attempts.");
                    recipient.result = DeliveryResult::Failed(
                        format!("Failed after 3 attempts: {}", msg));
                    continue;
                }
            }

            // Skip (and complete) if no MX servers
            if recipient.mx_servers.is_none() {
                debug!("(worker) delivery failed (no valid MX records).");
                recipient.result = DeliveryResult::Failed(
                    "MX records found but none are valid".to_owned());
                continue;
            }

            // Sequence through MX servers
            let mx_servers: &Vec<SocketAddr> = recipient.mx_servers.as_ref().unwrap();

            for i in recipient.current_mx .. mx_servers.len() {

                // Mark completed if this MX server is known to have been successfully
                // delivered to already
                if internal_status.delivered_to_mx.contains(&mx_servers[i]) {
                    debug!("(worker) delivery skipped (was delivered with other recipients).");
                    recipient.result = DeliveryResult::Delivered(
                        "[was delivered along with another recipients delivery]".to_owned());
                    continue 'next_recipient;
                }

                // Attempt delivery to this MX server
                recipient.result = ::worker::delivery::mx_delivery(
                    &internal_status.rfc_email, internal_status.message_id.clone(),
                    &mx_servers[i], &*self.helo_name, attempt);

                match recipient.result {
                    DeliveryResult::Delivered(_) => {
                        // save in delivered_to_mx list
                        internal_status.delivered_to_mx.push(mx_servers[i].clone());
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

            if let DeliveryResult::Deferred(a, _) = recipient.result {
                some_deferred_with_attempts = Some(a);
            }
        }

        // Lock the storage
        let mut guard = match (*self.storage).write() {
            Ok(guard) => guard,
            Err(e) => {
                error!("{:?}", e);
                return WorkerStatus::LockPoisoned;
            },
        };

        // Store the email delivery result (so far) into storage
        if let Err(e) = (*guard).store(&internal_status) {
            error!("{:?}", e);
            return WorkerStatus::StorageWriteFailed;
        }

        if let Some(attempts) = some_deferred_with_attempts {
            // Create a new worker task to retry later
            self.tasks.insert( Task {
                tasktype: TaskType::Resend,
                time: Instant::now() + Duration::from_secs(60 * 3u64.pow(attempts as u32)),
                message_id: internal_status.message_id.clone(),
            });
        }

        WorkerStatus::Ok
    }

    fn handle_task(&mut self, task: &Task) -> WorkerStatus {
        match task.tasktype {
            TaskType::Resend => {
                debug!("(worker) resending a deferred email");
                let internal_status = {
                    let guard = match (*self.storage).read() {
                        Ok(guard) => guard,
                        Err(_) => return WorkerStatus::LockPoisoned,
                    };
                    match (*guard).retrieve(&*task.message_id) {
                        Err(e) => {
                            warn!("Unable to retrieve task: {:?}", e);
                            return WorkerStatus::Ok
                        },
                        Ok(internal_status) => internal_status
                    }
                };
                return self.send_email(internal_status);
            },
        }
    }
}

// Get MX records for email recipients
fn get_mx_records_for_email(internal_status: &mut InternalStatus)
{
    use std::net::{SocketAddr, ToSocketAddrs};

    // Look-up the MX records for each recipient
    for recipient in &mut internal_status.recipients {
        let mx_records = match get_mx_records_for_domain(&*recipient.domain) {
            Err(e) => {
                recipient.result = DeliveryResult::Failed(
                    format!("Unable to fetch MX record: {:?}", e));
                warn!("MX LOOKUP FAILED FOR {}", recipient.email_addr);
                continue;
            }
            Ok(records) => {
                let mut mx_records: Vec<SocketAddr> = Vec::new();
                for record in records {
                    match (&*record, 25_u16).to_socket_addrs() {
                        Err(_) => {
                            warn!("ToSocketAddr FAILED FOR {}: {}",
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
        debug!("DEBUG: got mx servers for {}: {:?}",
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
