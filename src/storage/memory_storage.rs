
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

pub struct Record {
    email: Email,
    status: InternalStatus,
    retrieved: bool,
}

pub struct MemoryStorage(HashMap<String, Record>);

impl MemoryStorage {
    pub fn new() -> MemoryStorage {
        MemoryStorage(HashMap::new())
    }
}

impl MailstromStorage for MemoryStorage {
    type Error = MemoryStorageError;

    fn store(&mut self, email: Email, internal_status: InternalStatus)
             -> Result<(), MemoryStorageError>
    {
        self.0.insert(internal_status.message_id.clone(), Record {
            email: email,
            status: internal_status,
            retrieved: false
        });
        Ok(())
    }

    fn update_status(&mut self, internal_status: &InternalStatus)
                     -> Result<(), MemoryStorageError>
    {
        let record: &mut Record = match self.0.get_mut(&internal_status.message_id) {
            None => return Err(MemoryStorageError::NotFound),
            Some(record) => record,
        };

        record.status = internal_status.clone();
        Ok(())
    }

    fn retrieve(&self, message_id: &str) -> Result<(Email, InternalStatus), MemoryStorageError>
    {
        let record: &Record = match self.0.get(message_id) {
            None => return Err(MemoryStorageError::NotFound),
            Some(record) => record,
        };
        Ok((record.email.clone(), record.status.clone()))
    }

    fn retrieve_status(&self, message_id: &str) -> Result<InternalStatus, MemoryStorageError>
    {
        let record: &Record = match self.0.get(message_id) {
            None => return Err(MemoryStorageError::NotFound),
            Some(record) => record,
        };
        Ok(record.status.clone())
    }

    fn retrieve_all_incomplete(&self) -> Result<Vec<InternalStatus>, Self::Error>
    {
        Ok(self.0.values()
           .filter_map(|record| {
               if record.status.attempts_remaining==0 { None }
               else { Some(record.status.clone()) }
           })
           .collect())
    }

    fn retrieve_all_recent(&mut self) -> Result<Vec<InternalStatus>, Self::Error>
    {
        Ok(self.0.values_mut()
           .filter_map(|record| {
               if record.status.attempts_remaining==0 {
                   if record.retrieved==true {
                       None
                   } else {
                       record.retrieved = true;
                       Some(record.status.clone())
                   }
               }
               else {
                   Some(record.status.clone())
               }
           })
           .collect())

    }
}
