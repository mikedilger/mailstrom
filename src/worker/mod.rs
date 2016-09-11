
use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicBool, Ordering};

pub enum Message {
    /// Ask the worker to terminate
    Terminate,
}

pub struct Worker
{
    pub receiver: mpsc::Receiver<Message>,

    // Whether or not we are dead.  We only write this, and since the arc
    // is shared with Mailstrom, Mailstrom can then know that we died.
    dead: Arc<AtomicBool>,
}

impl Worker
{
    pub fn new(receiver: mpsc::Receiver<Message>, dead: Arc<AtomicBool>) -> Worker {
        Worker {
            receiver: receiver,
            dead: dead,
        }
    }

    pub fn run(&mut self) {
        loop {
            // Receive a message
            let message = match self.receiver.recv() {
                Ok(message) => message,
                Err(_) => return, // DIE due to ERROR
            };

            match message {
                Message::Terminate => {
                    println!("Terminating");
                    self.dead.store(true, Ordering::SeqCst);
                    return; // DIE on request
                },
            }
        }
    }
}
