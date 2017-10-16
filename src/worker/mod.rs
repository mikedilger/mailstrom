
mod smtp;
mod task;
mod mx;

use std::sync::{Arc, RwLock};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::atomic::{AtomicU8, Ordering};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use trust_dns_resolver::Resolver;

use prepared_email::PreparedEmail;
use message_status::InternalMessageStatus;
use delivery_result::DeliveryResult;
use storage::MailstromStorage;
use self::task::{Task, TaskType};
use ::Config;

const LOOP_DELAY: u64 = 10;

pub enum Message {
    /// Start sending emails
    Start,
    /// Ask the worker to deliver an email (message_id is provided, Mailstrom will have
    /// already stored it)
    SendEmail(String),
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
    StorageReadFailed = 5,
    ResolverCreationFailed = 6,
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
            5 => WorkerStatus::StorageReadFailed,
            6 => WorkerStatus::ResolverCreationFailed,
            _ => WorkerStatus::Unknown,
        }
    }
}

pub struct Worker<S: MailstromStorage + 'static>
{
    pub receiver: mpsc::Receiver<Message>,

    worker_status: Arc<AtomicU8>,

    config: Config,

    // Persistent shared storage
    storage: Arc<RwLock<S>>,

    // A list of tasks we need to do later, sorted in time order
    tasks: BTreeSet<Task>,

    paused: bool,
}

impl<S: MailstromStorage + 'static> Worker<S>
{
    pub fn new(receiver: mpsc::Receiver<Message>,
               storage: Arc<RwLock<S>>,
               worker_status: Arc<AtomicU8>,
               config: Config)
               -> Worker<S>
    {
        let mut worker = Worker {
            receiver: receiver,
            worker_status: worker_status,
            config: config,
            storage: storage,
            tasks: BTreeSet::new(),
            paused: true,
        };

        // Load the incomplete (queued and/or deferred) email statuses, for tasking
        if let Ok(guard) = (*worker.storage).write() {
            if let Ok(mut isvec) = (*guard).retrieve_all_incomplete() {
                // Create one task for each queued/deferred email
                for is in isvec.drain(..) {
                    worker.tasks.insert( Task {
                        tasktype: TaskType::Resend,
                        time: Instant::now(),
                        message_id: is.message_id.clone(),
                    });
                }
            } else {
                worker.worker_status.store(WorkerStatus::StorageReadFailed as u8,
                                           Ordering::SeqCst);
            }
        } else {
            worker.worker_status.store(WorkerStatus::LockPoisoned as u8,
                                       Ordering::SeqCst);
        }

        worker
    }

    pub fn run(&mut self) {

        let resolver: Resolver = match Resolver::new(
            self.config.resolver_config.clone(),
            self.config.resolver_opts.clone())
        {
            Ok(resolver) => resolver,
            Err(e) => {
                self.worker_status.store(WorkerStatus::ResolverCreationFailed as u8,
                                         Ordering::SeqCst);
                info!("(worker) failed and terminated: {:?}", e);
                return;
            }
        };

        loop {
            // Compute the timeout
            // This timeout represents how long we wait for a message.  If there are any
            // tasks in the tasklist (and we are not paused), this will be the time until
            // the first task is due.  Otherwise it is set to LOOP_DELAY seconds.
            let timeout: Duration = if self.paused {
                debug!("(worker) loop start (paused)");
                Duration::from_secs(LOOP_DELAY)
            }
            else if let Some(task) = self.tasks.iter().next() {
                debug!("(worker) loop start (tasks in queue)");
                let now = Instant::now();
                if task.time > now {
                    task.time - now
                } else {
                    Duration::new(0,0) // overdue!
                }
            } else {
                debug!("(worker) loop start (no tasks)");
                Duration::from_secs(LOOP_DELAY)
            };

            debug!("(worker) waiting for a message ({} seconds)", timeout.as_secs());

            // Receive a message.  Waiting at most until the time when the next task
            // is due, or LOOP_DELAY seconds if there are no tasks
            match self.receiver.recv_timeout(timeout) {
                Ok(message) => match message {
                    Message::Start => {
                        trace!("(worker) starting");
                        self.paused = false;
                    },
                    Message::SendEmail(message_id) => {
                        debug!("(worker) received SendEmail command");
                        // Create a task (don't do it right away) so we can more easily
                        // code pause-continue logic and eventually multiple worker threads
                        self.tasks.insert( Task {
                            tasktype: TaskType::Resend,
                            time: Instant::now(),
                            message_id: message_id,
                        });
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

            if ! self.paused {
                // Copy out all the tasks that are due
                let now = Instant::now();
                let due_tasks: Vec<Task> = self.tasks.iter()
                    .filter(|t| now > t.time).cloned().collect();

                // Handle all these due tasks
                for task in &due_tasks {
                    let worker_status = self.handle_task(task, &resolver);
                    if worker_status != WorkerStatus::Ok {
                        self.worker_status.store(worker_status as u8,
                                                 Ordering::SeqCst);
                        debug!("(worker) failed and terminated");
                        return;
                    }
                    self.tasks.remove(task);
                }
            }
        }
    }

    fn handle_task(&mut self, task: &Task, resolver: &Resolver) -> WorkerStatus {
        match task.tasktype {
            TaskType::Resend => {
                debug!("(worker) resending a (queued/deferred) email");
                let (email, internal_message_status) = {
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
                return self.send_email(email, internal_message_status, resolver);
            },
        }
    }

    fn send_email(&mut self,
                  email: PreparedEmail,
                  mut internal_message_status: InternalMessageStatus,
                  resolver: &Resolver)
                  -> WorkerStatus
    {
        let mut need_mx: bool = false;
        for recipient in &internal_message_status.recipients {
            if recipient.mx_servers.is_none() {
                need_mx=true;
                break;
            }
        }

        if need_mx {
            ::worker::mx::get_mx_records_for_email(&mut internal_message_status,
                                                   &resolver);

            // Update storage with this MX information
            let status = self.update_status(&internal_message_status);
            if status != WorkerStatus::Ok {
                return status;
            }
        }

        // Fail all recipients after too many worker attempts
        if internal_message_status.attempts_remaining == 0 {
            for recipient in internal_message_status.recipients.iter_mut() {
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
        if deliver(&email, &mut internal_message_status, &self.config) {
            internal_message_status.attempts_remaining = 0;
        } else {
            internal_message_status.attempts_remaining =
                internal_message_status.attempts_remaining - 1;
        }

        // Update storage with the new delivery results
        let status = self.update_status(&internal_message_status);
        if status != WorkerStatus::Ok {
            return status;
        }

        if internal_message_status.attempts_remaining > 0 {
            let attempt = 3 - internal_message_status.attempts_remaining;
            // exponential backoff
            let delay = Duration::from_secs(self.config.base_resend_delay_secs
                                            * 3u64.pow(attempt as u32));
            trace!("Queueing task to retry {} in {} seconds",
                   &internal_message_status.message_id, delay.as_secs());

            // Create a new worker task to retry later
            self.tasks.insert( Task {
                tasktype: TaskType::Resend,
                time: Instant::now() + delay,
                message_id: internal_message_status.message_id.clone(),
            });
        }

        WorkerStatus::Ok
    }

    fn update_status(&mut self, internal_message_status: &InternalMessageStatus)
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

        if let Err(e) = (*guard).update_status(internal_message_status.clone()) {
            error!("{:?}", e);
            return WorkerStatus::StorageWriteFailed;
        }

        WorkerStatus::Ok
    }
}

struct MxDelivery {
    mx_server: String, // domain name
    recipients: Vec<usize> // index into InternalMessageStatus.recipients
}

// Organize delivery for one-SMTP-delivery per MX server, and then use smtp_deliver()
// Returns true only if all recipient deliveries have been completed (rather than deferred)
fn deliver(email: &PreparedEmail, internal_message_status: &mut InternalMessageStatus,
           config: &Config) -> bool
{
    let mut deferred_some: bool = false;

    // We will sort our deliver plans by MX server. Currently they are sorted
    // by recipient.
    let mut mx_delivery: Vec<MxDelivery> = Vec::new();

    for r_index in 0..internal_message_status.recipients.len() {

        let recip = &mut internal_message_status.recipients[r_index];

        // Skip this recipient if already completed
        match recip.result {
            DeliveryResult::Delivered(_) => continue,
            DeliveryResult::Failed(_) => continue,
            _ => {}
        }

        // If recipient was deferred too many times, fail them and skip them
        let mut data: Option<(u8, String)> = None;
        if let DeliveryResult::Deferred(a, ref msg) = recip.result {
            data = Some((a, msg.clone()));
        };
        if data.is_some() {
            let (attempts, msg) = data.unwrap();
            // We allow 5 attempts (even though worker does 3 passes, we might try
            // across multiple MX servers)
            if attempts >= 5 {
                debug!("(worker) delivery failed after 5 attempts.");
                recip.result = DeliveryResult::Failed(
                    format!("Failed after 5 attempts: {}", msg));
                continue;
            }
        }

        // Skip (and complete) if no MX servers
        if recip.mx_servers.is_none() {
            debug!("(worker) delivery failed (no valid MX records).");
            recip.result = DeliveryResult::Failed(
                "MX records found but none are valid".to_owned());
            continue;
        }

        // Sequence through this recipients MX servers
        let mx_servers: &Vec<String> = recip.mx_servers.as_ref().unwrap();

        // Add to our MxDelivery vector
        for i in recip.current_mx .. mx_servers.len() {
            // Find the index of the MX server in our mx_delivery array
            let maybe_position = mx_delivery.iter().position(
                |ref mxd| mxd.mx_server == mx_servers[i]);
            match maybe_position {
                None => {
                    // Add this new MX server with the current recipient
                    mx_delivery.push(MxDelivery {
                        mx_server: mx_servers[i].clone(),
                        recipients: vec![ r_index ],
                    });
                },
                Some(index) => {
                    // Add this recipient to the mx_delivery
                    mx_delivery[index].recipients.push( r_index );
                },
            }
        }
    }

    // Deliver on a per-mx basis
    for mxd in &mut mx_delivery {

        // Per-MX version of the prepared email
        let mut mx_prepared_email = email.clone();

        // Rebuild the 'To:' list; only add recipients for *this* MX server,
        // and for which delivery has not already completed
        mx_prepared_email.to = mxd.recipients.iter()
            .filter_map(|r| {
                if internal_message_status.recipients[*r].result.completed() {
                    None
                } else {
                    Some(internal_message_status.recipients[*r].smtp_email_addr.clone())
                }
            })
            .collect();

        // Skip this MX server if no addresses to deliver to
        // (this can happen if a previous server already handled its recipients and
        // the filter_map above removed them all)
        if mx_prepared_email.to.len() == 0 {
            continue;
        }

        // Actually deliver to this SMTP server
        // (we set attempt=1 but this gets replaced per recipient below)
        let result = ::worker::smtp::smtp_delivery(
            &mx_prepared_email, &*mxd.mx_server, config, 1);

        for r in mxd.recipients.iter() {

            // If the result is deferred, and the previous result was deferred, then
            // bump the attempt number and update the reason message
            if let DeliveryResult::Deferred(_, ref newmsg) = result {
                deferred_some = true;
                let mut data: Option<u8> = None;
                if let DeliveryResult::Deferred(attempts, _) = internal_message_status.recipients[*r].result
                {
                    data = Some(attempts);
                }
                if data.is_some() {
                    let attempts = data.unwrap();
                    internal_message_status.recipients[*r].result = DeliveryResult::Deferred(
                        attempts + 1, newmsg.clone());
                    continue;
                }
            }

            // For everyone else, just take the result
            internal_message_status.recipients[*r].result = result.clone();
        }

    }

    !deferred_some
}

pub fn is_ip(s: &str) -> bool
{
    if let Some(last) = s.chars().rev().next() {
        last.is_digit(10)
    } else {
        false
    }
}
