use crate::delivery_result::DeliveryResult;

/// Per-Recipient Delivery Information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalRecipientStatus {
    /// The recipient's email address (for display)
    pub email_addr: String,

    /// The recipient's email address (for SMTP)
    pub smtp_email_addr: String,

    /// The domain parsed off of the recipients email address
    pub domain: String,

    /// The MX servers for the domain (as domain names), in order of delivery
    /// preference. If this is None, they have not been determined yet (DNS
    /// lookups take time).
    pub mx_servers: Option<Vec<String>>,

    /// The index into the MX server we are currently trying next
    pub current_mx: usize,

    /// The delivery result (so far) for this recipient
    pub result: DeliveryResult,
}

impl InternalRecipientStatus {
    pub fn as_recipient_status(&self) -> RecipientStatus {
        RecipientStatus {
            recipient: self.email_addr.clone(),
            result: self.result.clone(),
        }
    }
}

/// Per-Recpiient Delivery Information
#[derive(Debug, Serialize, Deserialize)]
pub struct RecipientStatus {
    pub recipient: String,
    pub result: DeliveryResult,
}
