use chrono::{DateTime, Utc};
use sea_orm::prelude::DateTimeWithTimeZone;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize)]
pub struct BridgeTransactionsResponse {
    pub l1_tx_hash: String,
    pub l2_tx_hash: String,
    pub l1_block_height: Option<i64>,
    pub l2_block_height: Option<i64>,
    pub status: Option<i16>,
    pub nonce: i64,
    pub chain_id: i64,
    pub l1_token: Option<String>,
    pub l2_token: Option<String>,
    pub from: String,
    pub to_twine_address: Option<String>,
    pub amount: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Serialize, Debug)]
pub struct TransactionSuggestion {
    pub l1_hash: String,
    pub l2_hash: String,
    pub block_number: String,
    pub from: String,
    pub to: String,
    pub token_symbol: String,
    pub timestamp: DateTime<Utc>,
    pub r#type: String,
    pub url: String,
}

#[derive(Deserialize, Debug)]
pub struct QuickSearchParams {
    pub q: String,
    pub limit: Option<u64>,
}
