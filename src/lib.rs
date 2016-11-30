
#![feature(integer_atomics)]

extern crate uuid;
extern crate email_format;

#[cfg(test)]
mod tests;
mod worker;
pub mod error;
mod email;

use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread;
use std::ops::Drop;

use worker::{Worker, Message};
use error::Error;
use email::Email;

pub use worker::WorkerStatus;

pub struct Mailstrom
{
    sender: mpsc::Sender<Message>,
    worker_status: Arc<AtomicU8>,
}

impl Mailstrom
{
    /// Create a new Mailstrom instance for sending emails.
    pub fn new() -> Mailstrom
    {
        let (sender, receiver) = mpsc::channel();

        let worker_status = Arc::new(AtomicU8::new(WorkerStatus::Ok as u8));

        let mut worker = Worker::new(receiver, worker_status.clone());

        let _ = thread::spawn(move|| {
            worker.run();
        });

        Mailstrom {
            sender: sender,
            worker_status: worker_status,
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
}

impl Drop for Mailstrom {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Terminate);
    }
}
