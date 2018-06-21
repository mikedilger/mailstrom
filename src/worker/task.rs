use std::cmp::{Ord, Ordering, PartialEq, PartialOrd};
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub enum TaskType {
    Resend,
}

#[derive(Clone)]
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

impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        self.time.eq(&other.time)
    }
}

impl Eq for Task {}
