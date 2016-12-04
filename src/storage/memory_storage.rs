
use std::error::Error;
use std::fmt;
use std::collections::HashMap;
use internal_status::InternalStatus;
use storage::{MailstromStorage, MailstromStorageError};

#[derive(Debug)]
pub enum MemoryStorageError {
    NotFound
}
impl Error for MemoryStorageError {
    fn description(&self) -> &str {
        match *self {
            MemoryStorageError::NotFound => "Email not found"
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            _ => None
        }
    }
}
impl MailstromStorageError for MemoryStorageError { }

impl fmt::Display for MemoryStorageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Memory Storage Error: {}", self.description())
    }
}

pub struct MemoryStorage {
    emails: HashMap<String, InternalStatus>,
}

impl MemoryStorage {
    pub fn new() -> MemoryStorage {
        MemoryStorage {
            emails: HashMap::new(),
        }
    }
}

impl MailstromStorage for MemoryStorage {
    type Error = MemoryStorageError;

    fn store(&mut self, internal_status: &InternalStatus) -> Result<(), MemoryStorageError>
    {
        self.emails.insert(internal_status.message_id.clone(), internal_status.clone());
        Ok(())
    }

    fn retrieve(&self, message_id: &str) -> Result<InternalStatus, MemoryStorageError>
    {
        match self.emails.get(message_id) {
            Some(internal_status) => Ok(internal_status.clone()),
            None => Err(MemoryStorageError::NotFound),
        }
    }
}
