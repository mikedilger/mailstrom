
#![feature(integer_atomics)]

extern crate uuid;
extern crate email_format;
extern crate resolv;
extern crate lettre;
#[macro_use] extern crate log;

#[cfg(test)]
mod tests;

mod worker;
pub use worker::WorkerStatus;

pub mod error;

mod internal_status;

pub mod storage;

pub mod status;
pub use status::{Status, DeliveryResult};


use std::sync::{mpsc, Arc, RwLock};
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;
use std::ops::Drop;
use email_format::Email;

use worker::{Worker, Message};
use error::Error;
use internal_status::InternalStatus;
use storage::MailstromStorage;


pub struct Config
{
    pub helo_name: String
}

pub struct Mailstrom<S: MailstromStorage + 'static>
{
    config: Config,
    sender: mpsc::Sender<Message>,
    worker_status: Arc<AtomicU8>,
    storage: Arc<RwLock<S>>,
}

impl<S: MailstromStorage + 'static> Mailstrom<S>
{
    /// Create a new Mailstrom instance for sending emails.
    pub fn new(config: Config, storage: S) -> Mailstrom<S>
    {
        let (sender, receiver) = mpsc::channel();

        let storage = Arc::new(RwLock::new(storage));

        let worker_status = Arc::new(AtomicU8::new(WorkerStatus::Ok as u8));

        let mut worker = Worker::new(receiver, storage.clone(), worker_status.clone(), &*config.helo_name);

        let _ = thread::spawn(move|| {
            worker.run();
        });

        Mailstrom {
            config: config,
            sender: sender,
            worker_status: worker_status,
            storage: storage,
        }
    }

    /// Ask Mailstrom to die.  This is not required, you can simply let it fall out
    /// of scope and it will clean itself up.
    pub fn die(&mut self) -> Result<(), Error>
    {
        try!(self.sender.send(Message::Terminate));
        Ok(())
    }

    /// Determine the status of the worker
    pub fn worker_status(&self) -> WorkerStatus
    {
        WorkerStatus::from_u8(self.worker_status.load(Ordering::SeqCst))
    }

    /// Send an email, getting back it's message-id
    pub fn send_email(&mut self, email: Email) -> Result<String, Error>
    {
        let internal_status = try!(InternalStatus::from_email(
            email, &*self.config.helo_name));

        let message_id = internal_status.message_id.clone();

        try!(self.sender.send(Message::SendEmail(internal_status)));

        info!("Passed email {} off to worker", &*message_id);

        Ok(message_id)
    }

    // Query Status of email
    pub fn query_status(&mut self, message_id: &str) -> Result<Status, Error>
    {
        let guard = match (*self.storage).read() {
            Ok(guard) => guard,
            Err(_) => return Err(Error::Lock),
        };

        let email = try!((*guard).retrieve(message_id));

        Ok(email.as_status())
    }
}

impl<S: MailstromStorage + 'static> Drop for Mailstrom<S>
{
    fn drop(&mut self) {
        info!("Mailstrom is terminating.");
        let _ = self.sender.send(Message::Terminate);
    }
}
