
#[cfg(test)]
mod tests;
mod worker;

use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::ops::Drop;

use worker::{Worker, Message};

pub struct Mailstrom
{
    sender: mpsc::Sender<Message>,
    dead: Arc<AtomicBool>,
}

impl Mailstrom
{
    /// Create a new Mailstrom instance for sending emails.
    pub fn new() -> Mailstrom
    {
        let (sender, receiver) = mpsc::channel();

        let dead = Arc::new(AtomicBool::new(false));

        let mut worker = Worker::new(receiver, dead.clone());

        let _ = thread::spawn(move|| {
            worker.run();
        });

        Mailstrom {
            sender: sender,
            dead: dead,
        }
    }

    /// Ask Mailstrom to die.  This is not required, you can simply let it fall out
    /// of scope and it will clean itself up.
    pub fn die(&mut self) -> Result<(), mpsc::SendError<Message>>
    {
        try!(self.sender.send(Message::Terminate));
        Ok(())
    }

    /// Determine if Mailstrom is dead
    pub fn is_dead(&self) -> bool
    {
        self.dead.load(Ordering::SeqCst)
    }
}

impl Drop for Mailstrom {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Terminate);
    }
}
