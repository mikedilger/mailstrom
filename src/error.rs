
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
    Lock,
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

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        use ::std::error::Error as StdError;

        match *self {
            Error::Send(ref e) =>
                format!("{}: {:?}", self.description(), e).fmt(f),
            Error::EmailParser(ref e) =>
                format!("{}: {:?}", self.description(), e).fmt(f),
            Error::Storage(ref s) =>
                format!("{}: {}", self.description(), s).fmt(f),
            Error::Resolver(ref e) =>
                format!("{}: {:?}", self.description(), e).fmt(f),
            _ => format!("{}", self.description()).fmt(f),
        }
    }
}

impl ::std::error::Error for Error {
    fn description(&self) -> &str
    {
        match *self {
            Error::Send(_) => "Unable to send message to worker",
            Error::EmailParser(_) => "Email does not parse",
            Error::Storage(_) => "Could not store or retrieve email state data",
            Error::DnsUnavailable => "DNS unavailable",
            Error::Resolver(_) => "DNS error",
            Error::Lock => "Lock poisoned",
        }
    }

    fn cause(&self) -> Option<&::std::error::Error>
    {
        match *self {
            Error::Send(ref e) => Some(e),
            Error::EmailParser(ref e) => Some(e),
            Error::Resolver(ref e) => Some(e),
            _ => None,
        }
    }
}
