
use uuid::Uuid;
use email_format::Email;
use email_format::rfc5322::headers::Bcc;
use email_format::rfc5322::types::{AddressList, Address, GroupList, Mailbox};
use error::Error;
use recipient_status::{InternalRecipientStatus, RecipientStatus};
use delivery_result::DeliveryResult;

/// An email to be sent (internal format).  This is exposed publicly for
/// implementers of `MailstromStorage` but otherwise should not
/// be needed by users of this library.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalMessageStatus {
    /// The parsed-out (or generated) message ID
    pub message_id: String,

    /// The parsed-out list of recipients, and the state each is in.  If this
    /// is None, then the recipient information has not been determined yet
    /// (MX record lookups take some time).
    pub recipients: Vec<InternalRecipientStatus>,

    /// Attempts remaining. This counts backwards to zero. If all deliveries are
    /// complete (permanent success or failure), it is set to zero.
    ///
    /// Per-recipient deferred attempt numbers count upwards, and may get more
    /// attempts because a single worker pass may try a recipient on muliple MX
    /// servers.
    pub attempts_remaining: u8,
}

impl InternalMessageStatus
{
    pub fn create(mut email: Email, helo_name: &str)
                  -> Result<(InternalMessageStatus, Email), Error>
    {
        let message_id = match email.get_message_id() {
            Some(mid) => {
                format!("{}@{}", mid.0.id_left, mid.0.id_right)
            },
            None => {
                let message_id = format!(
                    "{}@{}",
                    Uuid::new_v4().hyphenated().to_string(),
                    helo_name);
                try!(email.set_message_id(&*format!("<{}>", message_id)));
                message_id
            }
        };

        let recipients = determine_recipients(&email);

        // Strip any Bcc header line (to make it blind)
        email.clear_bcc();

        Ok((InternalMessageStatus {
            message_id: message_id,
            recipients: recipients,
            attempts_remaining: 3,
        }, email))
    }

    pub fn as_message_status(&self) -> MessageStatus
    {
        MessageStatus {
            message_id: self.message_id.clone(),
            recipient_status: self.recipients.iter().map(|r| RecipientStatus {
                recipient: r.email_addr.clone(),
                result: r.result.clone(),
            }).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageStatus {
    pub message_id: String,
    pub recipient_status: Vec<RecipientStatus>,
}

impl MessageStatus {
    pub fn succeeded(&self) -> bool
    {
        self.recipient_status.iter().all(|ref r| {
            match r.result {
                DeliveryResult::Delivered(_) => true,
                _ => false
            }
        })
    }

    pub fn completed(&self) -> bool
    {
        self.recipient_status.iter().all(|ref r| {
            r.result.completed()
        })
    }
}




fn determine_recipients(email: &Email) -> Vec<InternalRecipientStatus>
{
    let mut recipients: Vec<InternalRecipientStatus> = Vec::new();

    if let Some(to) = email.get_to() {
        recipients.extend(address_list_recipients(to.0));
    }
    if let Some(cc) = email.get_cc() {
        recipients.extend(address_list_recipients(cc.0));
    }
    if let Some(bcc) = email.get_bcc() {
        if let Bcc::AddressList(al) = bcc {
            recipients.extend(address_list_recipients(al));
        }
    }

    recipients
}

fn address_list_recipients(address_list: AddressList) -> Vec<InternalRecipientStatus>
{
    let mut recipients: Vec<InternalRecipientStatus> = Vec::new();

    // extract out each recipient
    for address in address_list.0 {
        match address {
            Address::Mailbox(mb) => {
                recipients.push(recipient_from_mailbox(mb));
            },
            Address::Group(grp) => {
                if let Some(gl) = grp.group_list {
                    match gl {
                        GroupList::MailboxList(mbl) => {
                            for mb in mbl.0 {
                                recipients.push(recipient_from_mailbox(mb));
                            }
                        },
                        GroupList::CFWS(_) => continue,
                    }
                }
            },
        }
    }

    recipients
}

fn recipient_from_mailbox(mb: Mailbox) -> InternalRecipientStatus
{
    let (email_addr, smtp_email_addr, domain) = match mb {
        Mailbox::NameAddr(na) => (format!("{}", na),
                                  format!("{}", na.angle_addr.addr_spec),
                                  format!("{}", na.angle_addr.addr_spec.domain)),
        Mailbox::AddrSpec(ads) => (format!("{}", ads),
                                   format!("{}", ads),
                                   format!("{}", ads.domain)),
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
