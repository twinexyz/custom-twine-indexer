use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlockData {
    pub number: u64,
    pub timestamp: u64,
}
