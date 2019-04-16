use std::cmp::{Ord, Ordering, PartialOrd};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TaskType {
    Resend,
}

#[derive(Clone, PartialEq)]
pub struct Task {
    pub tasktype: TaskType,
    pub time: Instant,
    pub message_id: String,
}

impl Ord for Task {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time)
    }
}

impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.time.partial_cmp(&other.time)
    }
}

impl Eq for Task {}
