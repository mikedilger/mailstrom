use std::net::SocketAddr;
use std::time::Duration;
use lettre::transport::smtp::{SmtpTransportBuilder, SecurityLevel};
use lettre::transport::smtp::response::Severity;
use lettre::transport::smtp::error::Error as LettreSmtpError;
use lettre::transport::EmailTransport;
use lettre::email::Envelope as LettreEnvelope;
use lettre::email::SendableEmail;
use status::DeliveryResult;
use email_format::Email;
use email_format::rfc5322::types::Mailbox;
use ::Config;

pub struct Envelope<'a> {
    pub message_id: String,
    pub lettre_envelope: LettreEnvelope,
    pub email: &'a Email,
}

impl<'a> Envelope<'a> {
    pub fn new(email: &'a Email, message_id: String, to_addresses: Vec<String>)
               -> Envelope<'a>
    {
        let f = match email.get_sender() {
            // Use sender if available
            Some(sender) => match sender.0 {
                Mailbox::NameAddr(ref na) =>
                    format!("{}", na.angle_addr.addr_spec).trim().to_owned(),
                Mailbox::AddrSpec(ref aspec) =>
                    format!("{}", aspec).trim().to_owned(),
            },
            None => match (email.get_from().0).0[0] {
                Mailbox::NameAddr(ref na) =>
                    format!("{}", na.angle_addr.addr_spec).trim().to_owned(),
                Mailbox::AddrSpec(ref aspec) =>
                    format!("{}", aspec).trim().to_owned(),
            },
        };

        Envelope {
            message_id: message_id,
            lettre_envelope: LettreEnvelope {
                to: to_addresses,
                from: f,
            },
            email: email
        }
    }
}

impl<'a> SendableEmail for Envelope<'a> {
    fn envelope(&self) -> &LettreEnvelope {
        &self.lettre_envelope
    }
    fn message_id(&self) -> String {
        format!("{}", self.message_id)
    }
    fn message(self) -> String {
        format!("{}", self.email)
    }
}

// Deliver an email to an SMTP server
pub fn smtp_delivery<'a>(envelope: Envelope<'a>,
                         smtp_server: &SocketAddr, config: &Config, attempt: u8)
                         -> DeliveryResult
{
    trace!("SMTP delivery to [{}] at {}",
           envelope.lettre_envelope.to.join(","),
           smtp_server);

    let mailer = match SmtpTransportBuilder::new( smtp_server ) {
        Ok(m) => m,
        Err(e) => {
            return DeliveryResult::Failed(
                format!("Unable to setup SMTP transport: {:?}", e));
        }
    };

    // Configure the mailer
    let mut mailer = mailer.hello_name( &*config.helo_name )
        .security_level(SecurityLevel::Opportunistic) // STARTTLS if available
        .smtp_utf8(true) // is only used if the server supports it
        .timeout(Some(Duration::from_secs( config.smtp_timeout_secs )))
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
