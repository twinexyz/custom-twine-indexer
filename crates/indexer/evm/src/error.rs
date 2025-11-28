#[derive(Debug)]
pub enum ParserError {
    MissingTransactionHash,
    MissingBlockNumber,
    MissingBlockTimestamp,
    InvalidBlockTimestamp,
    DecodeError {
        event_type: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    UnknownEvent {
        signature: alloy_primitives::B256,
    },
    BatchNotFound {
        start_block: u64,
        end_block: u64,
    },
    FinalizedBeforeCommit {
        start_block: u64,
        end_block: u64,
        batch_hash: String,
    },
    NumberOverflow {
        value: u64,
    },
    SkipLog,
}

impl std::error::Error for ParserError {}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserError::MissingTransactionHash => write!(f, "Missing transaction hash in log"),
            ParserError::MissingBlockNumber => write!(f, "Missing block number in log"),
            ParserError::MissingBlockTimestamp => write!(f, "Missing block timestamp in log"),
            ParserError::InvalidBlockTimestamp => write!(f, "Invalid block timestamp"),
            ParserError::DecodeError { event_type, source } => {
                write!(f, "Failed to decode {} event: {}", event_type, source)
            }
            ParserError::UnknownEvent { signature } => {
                write!(f, "Unknown event type: {}", signature)
            }
            ParserError::BatchNotFound { start_block, end_block } => write!(
                f,
                "Batch not found for start_block: {}, end_block: {}",
                start_block, end_block
            ),
            ParserError::FinalizedBeforeCommit {
                start_block,
                end_block,
                batch_hash,
            } => write!(
                f,
                "Finalized event indexed before commit for start_block: {}, end_block: {}, batch_hash: {}",
                start_block, end_block, batch_hash
            ),
            ParserError::NumberOverflow { value } => write!(
                f,
                "Generated batch number {} exceeds i32 maximum value",
                value
            ),
            ParserError::SkipLog => write!(f, "Missing event in log")
        }
    }
}
