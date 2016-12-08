use std::net::SocketAddr;
use lettre::transport::smtp::{SmtpTransportBuilder, SecurityLevel};
use lettre::transport::smtp::response::Severity;
use lettre::transport::smtp::error::Error as LettreSmtpError;
use lettre::transport::EmailTransport;
use status::DeliveryResult;
use email_format::Email;
use email_format::rfc5322::types::Mailbox;

// Implement lettre's SendableEmail

pub struct Envelope<'a> {
    pub message_id: String,
    pub to_addresses: Vec<String>,
    pub email: &'a Email,
}

impl<'a> ::lettre::email::SendableEmail for Envelope<'a> {
    fn from_address(&self) -> String {
        match self.email.get_sender() {
            // Use sender if available
            Some(sender) => match sender.0 {
                Mailbox::NameAddr(ref na) =>
                    format!("{}", na.angle_addr.addr_spec).trim().to_owned(),
                Mailbox::AddrSpec(ref aspec) =>
                    format!("{}", aspec).trim().to_owned(),
            },
            None => match (self.email.get_from().0).0[0] {
                Mailbox::NameAddr(ref na) =>
                    format!("{}", na.angle_addr.addr_spec).trim().to_owned(),
                Mailbox::AddrSpec(ref aspec) =>
                    format!("{}", aspec).trim().to_owned(),
            },
        }
    }
    fn to_addresses(&self) -> Vec<String> {
        self.to_addresses.clone()
    }
    fn message(&self) -> String {
        format!("{}", self.email)
    }
    fn message_id(&self) -> String {
        format!("{}", self.message_id)
    }
}

// Deliver an email to an SMTP server
pub fn smtp_delivery<'a>(envelope: Envelope<'a>,
                         smtp_server: &SocketAddr, helo: &str, attempt: u8)
                         -> DeliveryResult
{
    trace!("SMTP delivery to [{}] at {}",
           envelope.to_addresses.join(","),
           smtp_server);

    let mailer = match SmtpTransportBuilder::new( smtp_server ) {
        Ok(m) => m,
        Err(e) => {
            return DeliveryResult::Failed(
                format!("Unable to setup SMTP transport: {:?}", e));
        }
    };

    // Configure the mailer
    let mut mailer = mailer.hello_name( helo )
        .security_level(SecurityLevel::Opportunistic) // STARTTLS if available
        .smtp_utf8(true) // is only used if the server supports it
        .build();

    let result = match mailer.send(envelope) {
        Ok(response) => {
            info!("(worker) delivery response: {:?}", response);
            match response.severity() {
                Severity::PositiveCompletion | Severity::PositiveIntermediate => {
                    DeliveryResult::Delivered( format!("{:?}", response) )
                },
                Severity::TransientNegativeCompletion => {
                    DeliveryResult::Deferred( attempt, format!("{:?}", response) )
                },
                Severity::PermanentNegativeCompletion => {
                    DeliveryResult::Failed( format!("{:?}", response) )
                },
            }
        },
        Err(LettreSmtpError::Transient(response)) => {
            info!("(worker) delivery failed response: {:?}", response);
            DeliveryResult::Deferred( attempt, format!("{:?}", response) )
        },
        Err(LettreSmtpError::Permanent(response)) => {
            info!("(worker) delivery failed response: {:?}", response);
            DeliveryResult::Failed( format!("{:?}", response) )
        },
        Err(LettreSmtpError::Resolution) => {
            info!("(worker) delivery failed: DNS resolution failed");
            DeliveryResult::Deferred( attempt, "DNS resolution failed".to_owned() )
        },
        // FIXME: certain LettreSmtpError::Io errors may also be transient.
        Err(e) => {
            info!("(worker) delivery failed response: {:?}", e);
            DeliveryResult::Failed( format!("{:?}", e) )
        },
    };

    mailer.close();

    result
}
