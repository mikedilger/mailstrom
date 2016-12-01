
use std::convert::From;
use std::sync::mpsc::SendError;
use email_format::rfc5322::ParseError;
use worker::Message;
use storage::MailstromStorageError;
use resolv::error::Error as ResolvError;

#[derive(Debug)]
pub enum Error {
    Send(SendError<Message>),
    EmailParser(ParseError),
    Storage(String),
    DnsUnavailable,
    Resolver(ResolvError),
}

impl From<SendError<Message>> for Error {
    fn from(e: SendError<Message>) -> Error
    {
        Error::Send(e)
    }
}

impl From<ParseError> for Error {
    fn from(e: ParseError) -> Error
    {
        Error::EmailParser(e)
    }
}

impl<S: MailstromStorageError> From<S> for Error {
    fn from(e: S) -> Error
    {
        Error::Storage(format!("{}", e))
    }
}

impl From<ResolvError> for Error {
    fn from(e: ResolvError) -> Error
    {
        Error::Resolver(e)
    }
}
