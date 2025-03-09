use crate::entities::native_token_deposit;

#[derive(Debug)]
pub enum DbModel {
    NativeTokenDeposit(native_token_deposit::ActiveModel),
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositMessage {
    pub nonce: i64,
    pub chain_id: i64,
    pub slot_number: i64,
    pub from_l1_pubkey: String,
    pub to_twine_address: String,
    pub l1_token: String,
    pub l2_token: String,
    pub amount: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositInfoResponse {
    pub deposit_count: i64,
    pub deposit_message: DepositMessage,
    pub timestamp: DateTime<Utc>,
}
