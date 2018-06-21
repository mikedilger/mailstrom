use delivery_result::DeliveryResult;
use recipient_status::{InternalRecipientStatus, RecipientStatus};

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

impl InternalMessageStatus {
    pub fn as_message_status(&self) -> MessageStatus {
        MessageStatus {
            message_id: self.message_id.clone(),
            recipient_status: self.recipients
                .iter()
                .map(|r| RecipientStatus {
                    recipient: r.email_addr.clone(),
                    result: r.result.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageStatus {
    pub message_id: String,
    pub recipient_status: Vec<RecipientStatus>,
}

impl MessageStatus {
    pub fn succeeded(&self) -> bool {
        self.recipient_status.iter().all(|r| match r.result {
            DeliveryResult::Delivered(_) => true,
            _ => false,
        })
    }

    pub fn completed(&self) -> bool {
        self.recipient_status.iter().all(|r| r.result.completed())
    }
}
