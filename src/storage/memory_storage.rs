
use std::error::Error;
use std::fmt;
use std::collections::HashMap;
use email::Email;
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
    emails: HashMap<String, Email>,
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

    fn store(&mut self, email: &Email) -> Result<(), MemoryStorageError>
    {
        self.emails.insert(email.message_id.clone(), email.clone());
        Ok(())
    }

    fn retrieve(&self, message_id: &str) -> Result<Email, MemoryStorageError>
    {
        match self.emails.get(message_id) {
            Some(email) => Ok(email.clone()),
            None => Err(MemoryStorageError::NotFound),
        }
    }
}
