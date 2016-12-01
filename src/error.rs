
use std::convert::From;
use std::sync::mpsc::SendError;
use email_format::rfc5322::ParseError;
use worker::Message;
use storage::MailstromStorageError;

#[derive(Debug)]
pub enum Error {
    Send(SendError<Message>),
    EmailParser(ParseError),
    Storage(String),
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
