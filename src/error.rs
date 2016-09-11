
use std::convert::From;
use std::sync::mpsc::SendError;
use email_format::rfc5322::ParseError;
use worker::Message;

#[derive(Debug)]
pub enum Error {
    Send(SendError<Message>),
    EmailParser(ParseError),
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
