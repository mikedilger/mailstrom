
use email::Email;

pub trait MailstromStorageError: ::std::error::Error { }

/// A trait for implementing Mailstrom storage
pub trait MailstromStorage: Send + Sync {
    type Error: MailstromStorageError;

    /// Store an `Email`.  This should overwrite if message-id matches an existing email.
    fn store(&mut self, email: &Email) -> Result<(), Self::Error>;

    /// Retrieve an `Email` based on the message_id
    fn retrieve(&self, message_id: &str) -> Result<Email, Self::Error>;
}
