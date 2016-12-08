
mod smtp;
mod task;
mod mx;

use std::sync::{Arc, RwLock};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::atomic::{AtomicU8, Ordering};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};
use std::net::SocketAddr;

use email_format::Email;
use internal_status::InternalStatus;
use status::DeliveryResult;
use storage::MailstromStorage;
use self::task::{Task, TaskType};
use self::smtp::Envelope;

pub enum Message {
    /// Ask the worker to deliver an email
    SendEmail(Email, InternalStatus),
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
                    Message::SendEmail(email, internal_status) => {
                        debug!("(worker) received SendEmail command");
                        let worker_status = self.send_email(email, internal_status, true);
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

    fn send_email(&mut self, email: Email, mut internal_status: InternalStatus, initial: bool)
                  -> WorkerStatus
    {
        if initial {
            // Initial storage of the email
            let status = self.update_storage(&email, &internal_status);
            if status != WorkerStatus::Ok {
                return status;
            }

            // Get MX records for each recipient
            ::worker::mx::get_mx_records_for_email(&mut internal_status);

            // Update storage with this MX information
            let status = self.update_storage(&email, &internal_status);
            if status != WorkerStatus::Ok {
                return status;
            }
        }

        // Fail all recipients after too many worker attempts
        if internal_status.attempts_remaining == 0 {
            for recipient in internal_status.recipients.iter_mut() {
                let mut data: Option<(u8, String)> = None;
                if let DeliveryResult::Deferred(attempts, ref msg) = recipient.result {
                    data = Some((attempts, msg.clone()));
                }
                if data.is_some() {
                    let (attempts,msg) = data.unwrap();
                    recipient.result = DeliveryResult::Failed(
                        format!("Too many attempts ({}): {}", attempts, msg));
                }
            }
        }

        // Attempt delivery of the email
        if deliver(&email, &mut internal_status, &*self.helo_name) {
            internal_status.attempts_remaining = 0;
        } else {
            internal_status.attempts_remaining = internal_status.attempts_remaining - 1;
        }

        // Update storage with the new delivery results
        let status = self.update_storage(&email, &internal_status);
        if status != WorkerStatus::Ok {
            return status;
        }

        if internal_status.attempts_remaining > 0 {
            let attempt = 3 - internal_status.attempts_remaining;
            let delay = Duration::from_secs(60 * 3u64.pow(attempt as u32));
            trace!("Queueing task to retry {} in {} seconds",
                   &internal_status.message_id, delay.as_secs());

            // Create a new worker task to retry later
            self.tasks.insert( Task {
                tasktype: TaskType::Resend,
                time: Instant::now() + delay,
                message_id: internal_status.message_id.clone(),
            });
        }

        WorkerStatus::Ok
    }

    fn update_storage(&mut self, email: &Email, internal_status: &InternalStatus)
       -> WorkerStatus
    {
        // Lock the storage
        let mut guard = match (*self.storage).write() {
            Ok(guard) => guard,
            Err(e) => {
                error!("{:?}", e);
                return WorkerStatus::LockPoisoned;
            },
        };

        // Store the email delivery result (so far) into storage
        if let Err(e) = (*guard).store(&email, &internal_status) {
            error!("{:?}", e);
            return WorkerStatus::StorageWriteFailed;
        }

        WorkerStatus::Ok
    }

    fn handle_task(&mut self, task: &Task) -> WorkerStatus {
        match task.tasktype {
            TaskType::Resend => {
                debug!("(worker) resending a deferred email");
                let (email, internal_status) = {
                    let guard = match (*self.storage).read() {
                        Ok(guard) => guard,
                        Err(_) => return WorkerStatus::LockPoisoned,
                    };
                    match (*guard).retrieve(&*task.message_id) {
                        Err(e) => {
                            warn!("Unable to retrieve task: {:?}", e);
                            return WorkerStatus::Ok
                        },
                        Ok(x) => x
                    }
                };
                return self.send_email(email, internal_status, false);
            },
        }
    }
}

fn deliver(email: &Email, internal_status: &mut InternalStatus, helo_name: &str) -> bool
{
    let mut some_deferred_with_attempts: Option<u8> = None;

    let to_addresses: Vec<String> = internal_status.recipients.iter()
        .map(|r| r.email_addr.clone())
        .collect();

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

            /*
            // Mark completed if this MX server is known to have been successfully
            // delivered to already
            // THIS DOES NOT WORK DUE TO THE RECIPIENTS PREVIOUSLY NOT INCLUDING THIS
            // CURRENT RECIPIENT: SEE BUG #3
             */

            let envelope = Envelope {
                message_id: internal_status.message_id.clone(),
                to_addresses: to_addresses.clone(),
                email: email
            };

            // Attempt delivery to this MX server
            recipient.result = ::worker::smtp::smtp_delivery(
                envelope, &mx_servers[i], helo_name, attempt);

            match recipient.result {
                DeliveryResult::Deferred(_,_) => { } // continue MX loop
                _ => break, // Exit mx loop
            }
        }

        if let DeliveryResult::Deferred(a, _) = recipient.result {
            some_deferred_with_attempts = Some(a);
        }
    }

    some_deferred_with_attempts.is_some()
}
