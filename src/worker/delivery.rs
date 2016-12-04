use std::net::SocketAddr;
use lettre::transport::smtp::{SmtpTransportBuilder, SecurityLevel};
use lettre::transport::smtp::response::Severity;
use lettre::transport::smtp::error::Error as LettreSmtpError;
use lettre::transport::EmailTransport;
use status::DeliveryResult;
use email_format::Email;
use email_format::rfc5322::types::{Mailbox, Address, GroupList};

// Implement lettre's SendableEmail

struct SEmail<'a> {
    email: &'a Email,
    message_id: String,
}

impl<'a> ::lettre::email::SendableEmail for SEmail<'a> {
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
        let mut output: Vec<String> = Vec::new();

        if let Some(to) = self.email.get_to() {
            for addr in (to.0).0.iter() {
                match addr {
                    &Address::Mailbox(ref mb) => {
                        match mb {
                            &Mailbox::NameAddr(ref na) =>
                                output.push(format!("{}", na.angle_addr.addr_spec)
                                            .trim().to_owned()),
                            &Mailbox::AddrSpec(ref aspec) =>
                                output.push(format!("{}", aspec)
                                            .trim().to_owned()),
                        }
                    },
                    &Address::Group(ref grp) => {
                        match grp.group_list {
                            Some(GroupList::MailboxList(ref mbl)) => {
                                for mb in mbl.0.iter() {
                                    match mb {
                                        &Mailbox::NameAddr(ref na) =>
                                            output.push(format!("{}", na.angle_addr.addr_spec)
                                                        .trim().to_owned()),
                                        &Mailbox::AddrSpec(ref aspec) =>
                                            output.push(format!("{}", aspec)
                                                        .trim().to_owned()),
                                    }
                                }
                            },
                            _ => {},
                        }
                    },
                }
            }
        }
        output
    }
    fn message(&self) -> String {
        format!("{}", self.email)
    }
    fn message_id(&self) -> String {
        format!("{}", self.message_id)
    }
}

// Deliver an email to an MX server
pub fn mx_delivery(email: &Email, message_id: String, mx_server: &SocketAddr,
                   helo: &str, attempt: u8)
                   -> DeliveryResult
{
    let mailer = match SmtpTransportBuilder::new( mx_server ) {
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

    let semail = SEmail {
        email: email,
        message_id: message_id,
    };;

    let result = match mailer.send(semail) {
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
