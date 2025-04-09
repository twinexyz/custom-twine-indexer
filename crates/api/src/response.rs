use sea_orm::prelude::DateTimeWithTimeZone;

#[derive(Debug, Clone, serde::Serialize)]
pub struct L1DepositResponse {
    pub l1_tx_hash: String,
    pub l2_tx_hash: String,
    pub slot_number: Option<i64>,
    pub l2_slot_number: i64,
    pub block_number: Option<i64>,
    // pub status: i16,
    pub nonce: i64,
    pub chain_id: i64,
    pub l1_token: String,
    pub l2_token: String,
    pub from: String,
    pub to_twine_address: String,
    pub amount: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct L1WithdrawResponse {
    pub l1_tx_hash: String,
    pub l2_tx_hash: String,
    pub slot_number: Option<i64>,
    pub l2_slot_number: i64,
    pub block_number: Option<i64>,
    // pub status: i16,
    pub nonce: i64,
    pub chain_id: i64,
    pub l1_token: String,
    pub l2_token: String,
    pub from: String,
    pub to_twine_address: String,
    pub amount: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct L2WithdrawResponse {
    pub l1_tx_hash: String,
    pub l2_tx_hash: String,
    pub slot_number: Option<i64>,
    pub l2_slot_number: i64,
    pub block_number: Option<i64>,
    // pub status: i16,
    pub nonce: i64,
    pub chain_id: i64,
    pub l1_token: String,
    pub l2_token: String,
    pub from: String,
    pub to_twine_address: String,
    pub amount: String,
    pub created_at: DateTimeWithTimeZone,
}
