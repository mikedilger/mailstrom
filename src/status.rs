
use email::DeliveryResult;

#[derive(Debug)]
pub struct RecipientStatus {
    pub recipient: String,
    pub result: DeliveryResult,
}

#[derive(Debug)]
pub struct Status {
    pub message_id: String,
    pub recipient_status: Vec<RecipientStatus>,
}

impl Status {
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
            match r.result {
                DeliveryResult::Queued => false,
                DeliveryResult::Deferred(_,_) => false,
                _ => true,
            }
        })
    }
}
