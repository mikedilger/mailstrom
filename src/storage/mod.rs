pub mod memory_storage;
pub use self::memory_storage::MemoryStorage;

use crate::message_status::InternalMessageStatus;
use crate::prepared_email::PreparedEmail;

pub trait MailstromStorageError: ::std::error::Error {}

/// A trait for implementing Mailstrom storage
pub trait MailstromStorage: Send + Sync {
    type Error: MailstromStorageError;

    /// Store a `PreparedEmail`.  This should overwrite if message-id matches an existing
    /// email.
    fn store(
        &mut self,
        email: PreparedEmail,
        internal_message_status: InternalMessageStatus,
    ) -> Result<(), Self::Error>;

    /// Update the status of an email
    fn update_status(
        &mut self,
        internal_message_status: InternalMessageStatus,
    ) -> Result<(), Self::Error>;

    /// Retrieve a `PreparedEmail` and `InternalMessageStatus` based on the message_id
    fn retrieve(
        &self,
        message_id: &str,
    ) -> Result<(PreparedEmail, InternalMessageStatus), Self::Error>;

    /// Retrieve an `InternalMessageStatus` based on the message_id
    fn retrieve_status(&self, message_id: &str) -> Result<InternalMessageStatus, Self::Error>;

    /// Retrieve all incomplete emails (status only). This is used to continue retrying
    /// after shutdown and later startup.
    fn retrieve_all_incomplete(&self) -> Result<Vec<InternalMessageStatus>, Self::Error>;

    /// Retrieve all incomplete emails as well as all complete emails that have become
    /// complete since the last time this function was called. This can be implemented
    /// by storing a retrieved boolean as falswe when update_status saves as complete,
    /// and setting that boolean to true when this function is run.
    fn retrieve_all_recent(&mut self) -> Result<Vec<InternalMessageStatus>, Self::Error>;
}
