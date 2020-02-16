use crate::delivery_result::DeliveryResult;
use email_format::rfc5322::headers::Bcc;
use email_format::rfc5322::types::{Address, GroupList, Mailbox};
use email_format::Email;
use crate::error::Error;
use lettre::{EmailAddress, SendableEmail, Envelope};
use crate::message_status::InternalMessageStatus;
use crate::recipient_status::InternalRecipientStatus;
use uuid::Uuid;

/// An email, prepared for delivery.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PreparedEmail {
    pub to: Vec<String>,
    pub from: String,
    pub message_id: String,
    pub message: Vec<u8>,
}

impl PreparedEmail {
    pub fn as_sendable_email(&self) -> Result<SendableEmail, lettre::error::Error> {
        let to: Result<Vec<EmailAddress>, lettre::error::Error> =
            self.to.iter().map(|s| EmailAddress::new(s.clone())).collect();
        let to = to?;

        Ok(SendableEmail::new(
            Envelope::new(
                Some(EmailAddress::new(self.from.clone())?),
                to)?,
            self.message_id.clone(),
            self.message.clone()
        ))
    }
}

pub fn prepare_email(
    mut email: Email,
    helo_name: &str,
) -> Result<(PreparedEmail, InternalMessageStatus), Error> {
    let recipients = determine_recipients(&email);

    // Blind the Bcc
    email.clear_bcc();

    let message_id = match email.get_message_id() {
        Some(mid) => format!("{}@{}", mid.0.id_left, mid.0.id_right),
        None => {
            // Generate message-id
            let message_id = format!("{}@{}", Uuid::new_v4().hyphenated().to_string(), helo_name);
            email.set_message_id(&*format!("<{}>", message_id))?;
            message_id
        }
    };

    let prepared_email = PreparedEmail {
        to: recipients
            .iter()
            .map(|r| r.smtp_email_addr.clone())
            .collect(),
        from: format!("{}", email.get_from().0),
        message_id: message_id.clone(),
        message: format!("{}", email).into_bytes(),
    };

    // Verify that lettre::SendableEmail will not give us errors later on
    // down the track
    let _ = ::lettre::EmailAddress::new(prepared_email.from.clone())?;
    prepared_email.to.iter()
        .try_for_each(|s| ::lettre::EmailAddress::new(s.clone()).map(|_|()))?;

    let internal_message_status = InternalMessageStatus {
        message_id,
        recipients,
        attempts_remaining: 3,
    };

    Ok((prepared_email, internal_message_status))
}

fn determine_recipients(email: &Email) -> Vec<InternalRecipientStatus> {
    let mut addresses: Vec<Address> = Vec::new();

    if let Some(to) = email.get_to() {
        addresses.extend((to.0).0);
    }
    if let Some(cc) = email.get_cc() {
        addresses.extend((cc.0).0);
    }
    if let Some(bcc) = email.get_bcc() {
        if let Bcc::AddressList(al) = bcc {
            addresses.extend(al.0);
        }
    }

    addresses.dedup();

    let mut recipients: Vec<InternalRecipientStatus> = Vec::new();

    for address in addresses {
        match address {
            Address::Mailbox(mb) => {
                recipients.push(recipient_from_mailbox(mb));
            }
            Address::Group(grp) => {
                if let Some(gl) = grp.group_list {
                    match gl {
                        GroupList::MailboxList(mbl) => {
                            for mb in mbl.0 {
                                recipients.push(recipient_from_mailbox(mb));
                            }
                        }
                        GroupList::CFWS(_) => continue,
                    }
                }
            }
        }
    }

    recipients
}

fn recipient_from_mailbox(mb: Mailbox) -> InternalRecipientStatus {
    let (email_addr, smtp_email_addr, domain) = match mb {
        Mailbox::NameAddr(na) => (
            format!("{}", na),
            format!("{}", na.angle_addr.addr_spec),
            format!("{}", na.angle_addr.addr_spec.domain),
        ),
        Mailbox::AddrSpec(ads) => (
            format!("{}", ads),
            format!("{}", ads),
            format!("{}", ads.domain),
        ),
    };

    InternalRecipientStatus {
        email_addr: email_addr.trim().to_owned(),
        smtp_email_addr: smtp_email_addr.trim().to_owned(),
        domain: domain.trim().to_owned(),
        mx_servers: None, // To be determined later by a worker task
        current_mx: 0,
        result: DeliveryResult::Queued,
    }
}
