
use std::error::Error;
use std::fmt;
use std::collections::HashMap;
use email_format::Email;
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
    emails: HashMap<String, Email>,
    statuses: HashMap<String, InternalStatus>,
}

impl MemoryStorage {
    pub fn new() -> MemoryStorage {
        MemoryStorage {
            emails: HashMap::new(),
            statuses: HashMap::new(),
        }
    }
}

impl MailstromStorage for MemoryStorage {
    type Error = MemoryStorageError;

    fn store(&mut self, email: &Email, internal_status: &InternalStatus)
             -> Result<(), MemoryStorageError>
    {
        self.emails.insert(internal_status.message_id.clone(), email.clone());
        self.statuses.insert(internal_status.message_id.clone(), internal_status.clone());
        Ok(())
    }

    fn update_status(&mut self, internal_status: &InternalStatus)
                     -> Result<(), MemoryStorageError>
    {
        self.statuses.insert(internal_status.message_id.clone(), internal_status.clone());
        Ok(())
    }

    fn retrieve(&self, message_id: &str) -> Result<(Email, InternalStatus), MemoryStorageError>
    {
        let email = match self.emails.get(message_id) {
            Some(email) => email,
            None => return Err(MemoryStorageError::NotFound),
        };
        let status = match self.statuses.get(message_id) {
            Some(status) => status,
            None => return Err(MemoryStorageError::NotFound),
        };
        Ok((email.clone(), status.clone()))
    }

    fn retrieve_status(&self, message_id: &str) -> Result<InternalStatus, MemoryStorageError>
    {
        let status = match self.statuses.get(message_id) {
            Some(status) => status,
            None => return Err(MemoryStorageError::NotFound),
        };
        Ok(status.clone())
    }

    fn retrieve_all_incomplete(&self) -> Result<Vec<InternalStatus>, Self::Error>
    {
        Ok(self.statuses.values()
           .filter_map(|is| {
               if is.attempts_remaining == 0 {
                   None
               } else {
                   Some(is.clone())
               }
           })
           .collect())
    }
}
