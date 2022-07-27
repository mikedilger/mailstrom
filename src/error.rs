use crate::storage::MailstromStorageError;
use crate::worker::Message;
use email_format::rfc5322::ParseError;
use std::convert::From;
use std::io::Error as IoError;
use std::sync::mpsc::SendError;

#[derive(Debug)]
pub enum Error {
    Send(SendError<Message>),
    EmailParser(ParseError),
    General(String),
    Storage(String),
    DnsUnavailable,
    Lock,
    Io(IoError),
    LettreEmailAddress(lettre::error::Error),
}

impl From<SendError<Message>> for Error {
    fn from(e: SendError<Message>) -> Error {
        Error::Send(e)
    }
}

impl From<ParseError> for Error {
    fn from(e: ParseError) -> Error {
        Error::EmailParser(e)
    }
}

impl From<String> for Error {
    fn from(e: String) -> Error {
        Error::General(e)
    }
}

impl<S: MailstromStorageError> From<S> for Error {
    fn from(e: S) -> Error {
        Error::Storage(format!("{}", e))
    }
}

impl From<IoError> for Error {
    fn from(e: IoError) -> Error {
        Error::Io(e)
    }
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match *self {
            Error::Send(ref e) => write!(f, "Unable to send message to worker: {:?}", e),
            Error::EmailParser(ref e) => write!(f, "Email does not parse: {:?}", e),
            Error::General(ref e) => write!(f, "General error: {}", e),
            Error::Storage(ref s) => write!(f, "Could not store or retrieve email state data: {}", s),
            Error::DnsUnavailable => write!(f, "DNS unavailable"),
            Error::Lock => write!(f, "Lock poisoned"),
            Error::Io(ref e) => write!(f, "I/O Error: {}", e),
            Error::LettreEmailAddress(ref e) => write!(f, "Lettre crate Email Address error: {}", e),
        }
    }
}

impl ::std::error::Error for Error { }
