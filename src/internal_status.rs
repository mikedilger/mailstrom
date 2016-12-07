
use std::net::SocketAddr;
use uuid::Uuid;
use email_format::Email;
use email_format::rfc5322::headers::Bcc;
use email_format::rfc5322::types::{AddressList, Address, GroupList, Mailbox};
use error::Error;
use status::{Status, RecipientStatus, DeliveryResult};

/// Information about the recipients of an email to be sent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipient {
    /// The recipient's email address
    pub email_addr: String,

    /// The domain parsed off of the recipients email address
    pub domain: String,

    /// The MX servers for the domain, in order of delivery preference.
    /// If this is None, they have not been determined yet (DNS lookups take time).
    pub mx_servers: Option<Vec<SocketAddr>>,

    /// The index into the MX server we are currently trying next
    pub current_mx: usize,

    /// The delivery result (so far) for this recipient
    pub result: DeliveryResult,
}

/// An email to be sent (internal format).  This is exposed publicly for
/// implementers of `MailstromStorage`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalStatus {
    /// The parsed-out (or generated) message ID
    pub message_id: String,

    /// The parsed-out list of recipients, and the state each is in.  If this
    /// is None, then the recipient information has not been determined yet
    /// (MX record lookups take some time).
    pub recipients: Vec<Recipient>,
}

impl InternalStatus
{
    pub fn create(mut email: Email, helo_name: &str)
                  -> Result<(InternalStatus, Email), Error>
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

        Ok((InternalStatus {
            message_id: message_id,
            recipients: recipients,
        }, email))
    }

    pub fn as_status(&self) -> Status
    {
        Status {
            message_id: self.message_id.clone(),
            recipient_status: self.recipients.iter().map(|r| RecipientStatus {
                recipient: r.email_addr.clone(),
                result: r.result.clone(),
            }).collect(),
        }
    }
}

fn determine_recipients(email: &Email) -> Vec<Recipient>
{
    let mut recipients: Vec<Recipient> = Vec::new();

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

fn address_list_recipients(address_list: AddressList) -> Vec<Recipient>
{
    let mut recipients: Vec<Recipient> = Vec::new();

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

fn recipient_from_mailbox(mb: Mailbox) -> Recipient
{
    let (email_addr, domain) = match mb {
        Mailbox::NameAddr(na) => (format!("{}", na),
                                  format!("{}", na.angle_addr.addr_spec.domain)),
        Mailbox::AddrSpec(ads) => (format!("{}", ads),
                                   format!("{}", ads.domain)),
    };

    Recipient {
        email_addr: email_addr.trim().to_owned(),
        domain: domain.trim().to_owned(),
        mx_servers: None, // To be determined later by a worker task
        current_mx: 0,
        result: DeliveryResult::Queued,
    }
}
