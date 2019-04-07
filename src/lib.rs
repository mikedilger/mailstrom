//! Mailstrom handles email delivery in a background worker thread, with the following
//! features:
//!
//! * Accepts an email from the caller and then does everything necessary to get it
//!   delivered to all recipients without blocking the caller.
//! * Allows the caller to query the status of an earlier submitted email at any time,
//!   to determine if it is Queued, Delivered, Deferred, or has Failed, with details
//!   as to why, on a per-recipient basis.
//! * Handles all parsing, validation, and encoding of email content and headers,
//!   in compliance with RFC 5322 (and other RFCs).  Uses the
//!   [email-format](https://github.com/mikedilger/email-format) library for this.
//! * Either delivers via a configured SMTP relay, or directly to recipient mail servers
//!   by looking up the DNS MX record for each recipient and delivering directly to those
//!   Internet mail servers over SMTP.  Uses the
//!   [trust-dns](https://github.com/bluejekyll/trust-dns) library for DNS lookups.
//! * SMTP transport "heavy lifting" is performed via the [lettre](https://github.com/lettre/lettre)
//!   library.  Uses STARTTLS where available.
//! * Retries with exponential backoff for a fixed number of retries (currently fixed at 3),
//!   when the send result is Deferred
//! * Uses a pluggable user-defined state management (persistence) layer.
//!
//! ## Limitations
//!
//! * The [email-format](https://github.com/mikedilger/email-format) crate is somewhat incomplete
//!   and clunky still.  It doesn't incorporate RFC 6854 (updated From and Sender syntax) yet.
//!   It defines types one-to-one with ABNF parsing units, rather than as semantic units of meaning.
//!   And it doesn't let you use obvious types yet like setting the date from a `DateTime` type.
//!   However, these issues will be worked out in the near future.
//!
//! You can use it as follows:
//!
//! ```no_run
//! extern crate email_format;
//! extern crate mailstrom;
//!
//! use email_format::Email;
//! use mailstrom::Mailstrom;
//! use mailstrom::config::Config;
//! use mailstrom::storage::MemoryStorage;
//!
//! fn main() {
//!     let mut email = Email::new(
//!         "myself@mydomain.com",  // "From:"
//!         "Wed, 05 Jan 2015 15:13:05 +1300" // "Date:"
//!     ).unwrap();
//!
//!     email.set_bcc("myself@mydomain.com").unwrap();
//!     email.set_sender("from_myself@mydomain.com").unwrap();
//!     email.set_reply_to("My Mailer <no-reply@mydomain.com>").unwrap();
//!     email.set_to("You <you@yourdomain.com>, AndYou <andyou@yourdomain.com>").unwrap();
//!     email.set_cc("Our Friend <friend@frienddomain.com>").unwrap();
//!     email.set_subject("Hello Friend").unwrap();
//!     email.set_body("Good to hear from you.\r\n\
//!                     I wish you the best.\r\n\
//!                     \r\n\
//!                     Your Friend").unwrap();
//!
//!     let mut mailstrom = Mailstrom::new(
//!         Config {
//!             helo_name: "my.host.domainname".to_owned(),
//!             ..Default::default()
//!         },
//!         MemoryStorage::new());
//!
//!     // We must explicitly tell mailstrom to start actually sending emails.  If we
//!     // were only interested in reading the status of previously sent emails, we
//!     // would not send this command.
//!     mailstrom.start().unwrap();
//!
//!     let message_id = mailstrom.send_email(email).unwrap();
//!
//!     // Later on, after the worker thread has had time to process the request,
//!     // you can check the status:
//!
//!     let status = mailstrom.query_status(&*message_id).unwrap();
//!     println!("{:?}", status);
//! }
//! ```

extern crate email_format;
extern crate lettre;
extern crate failure;
extern crate trust_dns_resolver;
extern crate uuid;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate native_tls;

#[cfg(test)]
mod tests;

pub mod config;
use config::Config;

mod worker;
pub use worker::WorkerStatus;
use worker::{Message, Worker};

pub mod error;
use error::Error;

mod delivery_result;
pub use delivery_result::DeliveryResult;

mod recipient_status;
pub use recipient_status::RecipientStatus;

mod message_status;
pub use message_status::MessageStatus;

mod prepared_email;

pub mod storage;
use storage::MailstromStorage;

use email_format::Email;
use std::ops::Drop;
use std::sync::{mpsc, Arc, RwLock};
use std::thread;


pub struct Mailstrom<S: MailstromStorage + 'static> {
    config: Config,
    sender: mpsc::Sender<Message>,
    worker_status: Arc<RwLock<u8>>,
    storage: Arc<RwLock<S>>,
}

impl<S: MailstromStorage + 'static> Mailstrom<S> {
    /// Create a new Mailstrom instance for sending emails.
    pub fn new(config: Config, storage: S) -> Mailstrom<S> {
        let (sender, receiver) = mpsc::channel();

        let storage = Arc::new(RwLock::new(storage));

        let worker_status = Arc::new(RwLock::new(WorkerStatus::Ok as u8));

        let mut worker = Worker::new(
            receiver,
            Arc::clone(&storage),
            Arc::clone(&worker_status),
            config.clone(),
        );

        let _ = thread::spawn(move || {
            worker.run();
        });

        Mailstrom {
            config,
            sender,
            worker_status,
            storage,
        }
    }

    /// Mailstrom requires an explicit start command to start sending emails.  This is
    /// because some clients are only interested in reading the status of sent emails,
    /// and will terminate before any real sending can be accomplished.
    pub fn start(&mut self) -> Result<(), Error> {
        self.sender.send(Message::Start)?;
        Ok(())
    }

    /// Ask Mailstrom to die.  This is not required, you can simply let it fall out
    /// of scope and it will clean itself up.
    pub fn die(&mut self) -> Result<(), Error> {
        self.sender.send(Message::Terminate)?;
        Ok(())
    }

    /// Determine the status of the worker
    pub fn worker_status(&self) -> WorkerStatus {
        let ws = *self.worker_status.read().unwrap();
        WorkerStatus::from_u8(ws)
    }

    /// Send an email, getting back its message-id
    pub fn send_email(&mut self, email: Email) -> Result<String, Error> {
        let (prepared_email, internal_message_status) =
            crate::prepared_email::prepare_email(email, &*self.config.helo_name)?;

        let message_id = internal_message_status.message_id.clone();

        {
            // Lock the storage
            let mut guard = match (*self.storage).write() {
                Ok(guard) => guard,
                Err(_) => return Err(Error::Lock),
            };

            // Store the email
            (*guard).store(prepared_email, internal_message_status)?;
        }

        self.sender.send(Message::SendEmail(message_id.clone()))?;

        info!("Passed email {} off to worker", &*message_id);

        Ok(message_id)
    }

    // Query Status of email
    pub fn query_status(&mut self, message_id: &str) -> Result<MessageStatus, Error> {
        let guard = match (*self.storage).read() {
            Ok(guard) => guard,
            Err(_) => return Err(Error::Lock),
        };

        let status = (*guard).retrieve_status(message_id)?;

        Ok(status.as_message_status())
    }

    // Query recently queued and sent emails. This includes all emails where sending is not
    // yet complete, and also all emails where sending is complete but for which they have
    // not yet been reported on (via this function).
    pub fn query_recent(&mut self) -> Result<Vec<MessageStatus>, Error> {
        let mut guard = match (*self.storage).write() {
            Ok(guard) => guard,
            Err(_) => return Err(Error::Lock),
        };

        let vec_statuses = (*guard).retrieve_all_recent()?;
        Ok(vec_statuses.iter().map(|s| s.as_message_status()).collect())
    }
}

impl<S: MailstromStorage + 'static> Drop for Mailstrom<S> {
    fn drop(&mut self) {
        info!("Mailstrom is terminating.");
        let _ = self.sender.send(Message::Terminate);
    }
}
