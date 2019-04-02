use crate::message_status::InternalMessageStatus;
use crate::prepared_email::PreparedEmail;
use crate::storage::{MailstromStorage, MailstromStorageError};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum MemoryStorageError {
    NotFound,
}
impl Error for MemoryStorageError {
    fn description(&self) -> &str {
        match *self {
            MemoryStorageError::NotFound => "Email not found",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            _ => None,
        }
    }
}
impl MailstromStorageError for MemoryStorageError {}

impl fmt::Display for MemoryStorageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Memory Storage Error: {}", self.description())
    }
}

pub struct Record {
    email: PreparedEmail,
    status: InternalMessageStatus,
    retrieved: bool,
}

#[derive(Default)]
pub struct MemoryStorage(HashMap<String, Record>);

impl MemoryStorage {
    pub fn new() -> MemoryStorage {
        MemoryStorage(HashMap::new())
    }
}

impl MailstromStorage for MemoryStorage {
    type Error = MemoryStorageError;

    fn store(
        &mut self,
        email: PreparedEmail,
        internal_message_status: InternalMessageStatus,
    ) -> Result<(), MemoryStorageError> {
        self.0.insert(
            internal_message_status.message_id.clone(),
            Record {
                email,
                status: internal_message_status,
                retrieved: false,
            },
        );
        Ok(())
    }

    fn update_status(
        &mut self,
        internal_message_status: InternalMessageStatus,
    ) -> Result<(), MemoryStorageError> {
        let record: &mut Record = match self.0.get_mut(&internal_message_status.message_id) {
            None => return Err(MemoryStorageError::NotFound),
            Some(record) => record,
        };

        record.status = internal_message_status;
        Ok(())
    }

    fn retrieve(
        &self,
        message_id: &str,
    ) -> Result<(PreparedEmail, InternalMessageStatus), MemoryStorageError> {
        let record: &Record = match self.0.get(message_id) {
            None => return Err(MemoryStorageError::NotFound),
            Some(record) => record,
        };
        Ok((record.email.clone(), record.status.clone()))
    }

    fn retrieve_status(
        &self,
        message_id: &str,
    ) -> Result<InternalMessageStatus, MemoryStorageError> {
        let record: &Record = match self.0.get(message_id) {
            None => return Err(MemoryStorageError::NotFound),
            Some(record) => record,
        };
        Ok(record.status.clone())
    }

    fn retrieve_all_incomplete(&self) -> Result<Vec<InternalMessageStatus>, Self::Error> {
        Ok(self.0
            .values()
            .filter_map(|record| {
                if record.status.attempts_remaining == 0 {
                    None
                } else {
                    Some(record.status.clone())
                }
            })
            .collect())
    }

    fn retrieve_all_recent(&mut self) -> Result<Vec<InternalMessageStatus>, Self::Error> {
        Ok(self.0
            .values_mut()
            .filter_map(|record| {
                if record.status.attempts_remaining == 0 {
                    if record.retrieved {
                        None
                    } else {
                        record.retrieved = true;
                        Some(record.status.clone())
                    }
                } else {
                    Some(record.status.clone())
                }
            })
            .collect())
    }
}
