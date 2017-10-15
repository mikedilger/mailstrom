
/// The result (so far) of the sending of an email to a particular recipient
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeliveryResult {
    /// Mail is queued to be sent, but no attempt has yet been made to send. This state should
    /// be moved through rather quickly.
    Queued,

    /// Mail sending has been deferred due to a transient error. Number of attempts and Error
    /// are included.
    Deferred(u8, String),

    /// Mail has been sent. Delivery response included.
    Delivered(String),

    /// Mail sending has failed due to a permanent error. Error is included.
    Failed(String),
}

impl DeliveryResult {
    pub fn completed(&self) -> bool {
        match *self {
            DeliveryResult::Queued => false,
            DeliveryResult::Deferred(_,_) => false,
            _ => true
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecipientStatus {
    pub recipient: String,
    pub result: DeliveryResult,
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
