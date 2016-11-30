
use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

pub enum Message {
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

pub struct Worker
{
    pub receiver: mpsc::Receiver<Message>,

    worker_status: Arc<AtomicU8>,
}

impl Worker
{
    pub fn new(receiver: mpsc::Receiver<Message>, worker_status: Arc<AtomicU8>) -> Worker {
        Worker {
            receiver: receiver,
            worker_status: worker_status,
        }
    }

    pub fn run(&mut self) {

        let timeout: Duration = Duration::from_secs(60);

        loop {
            match self.receiver.recv_timeout(timeout) {
                Ok(message) => match message {
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
}
