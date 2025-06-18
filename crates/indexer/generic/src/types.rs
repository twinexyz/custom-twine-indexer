use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct EventContext {
    pub tx_hash: String,
    pub timestamp: DateTime<Utc>,
    pub chain_id: u64,
}

#[derive(Debug)]
pub struct BatchData {
    pub start_block: u64,
    pub end_block: u64,
    pub root_hash: Vec<u8>,
}

impl BatchData {
    pub fn length(&self) -> u64 {
        self.end_block - self.start_block + 1
    }

    pub fn batch_number(&self) -> u64 {
        self.start_block
    }
}

#[derive(Debug)]
pub enum EventHandlerError {
    BatchMismatch { expected: usize, actual: usize },
    InvalidEventData(String),
    DatabaseError(String),
    ParseError(String),
}

impl std::fmt::Display for EventHandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BatchMismatch { expected, actual } => {
                write!(
                    f,
                    "Batch length mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            Self::InvalidEventData(msg) => write!(f, "Invalid event data: {}", msg),
            Self::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for EventHandlerError {}