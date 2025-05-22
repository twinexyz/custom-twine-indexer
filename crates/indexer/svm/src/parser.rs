use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::BlockTransactions;
use base64::{engine::general_purpose, Engine as _};
use borsh::{BorshDeserialize, BorshSerialize};
use chrono::Utc;
use eyre::Result;
use num_traits::FromPrimitive;
use sea_orm::prelude::Decimal;
use sea_orm::{
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{debug, error, info};

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct DepositSuccessful {
    pub nonce: u64,
    pub from_l1_pubkey: String,
    pub to_twine_address: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: String,
    pub slot_number: u64,
    #[borsh(skip)]
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ForcedWithdrawSuccessful {
    pub nonce: u64,
    pub from_twine_address: String,
    pub to_l1_pub_key: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: String,
    pub slot_number: u64,
    #[borsh(skip)]
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FinalizeNativeWithdrawal {
    pub nonce: u64,
    pub receiver_l1_pubkey: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: u64,
    pub slot_number: u64,
    #[borsh(skip)]
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FinalizeSplWithdrawal {
    pub nonce: u64,
    pub receiver_l1_pubkey: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: u64,
    pub slot_number: u64,
    #[borsh(skip)]
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CommitBatch {
    pub start_block: u64,
    pub end_block: u64,
    pub chain_id: u64,
    pub slot_number: u64,
    #[borsh(skip)]
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FinalizedBatch {
    pub start_block: u64,
    pub end_block: u64,
    pub chain_id: u64,
    pub batch_hash: [u8; 32],
    #[borsh(skip)]
    pub signature: String,
    pub slot_number: u64,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FinalizedTransaction {
    pub start_block: u64,
    pub end_block: u64,
    pub chain_id: u64,
    pub deposit_count: u64,
    pub withdraw_count: u64,
    #[borsh(skip)]
    pub signature: String,
    pub slot_number: u64,
}

pub fn generate_number(start_block: u64, end_block: u64) -> Result<i32> {
    let input = format!("{}:{}", start_block, end_block);
    let digest = blake3::hash(input.as_bytes());
    let value = u64::from_le_bytes(digest.as_bytes()[..8].try_into().unwrap());
    let masked_value = value & 0x7FFF_FFFF;
    if masked_value > i32::MAX as u64 {
        Err(eyre::eyre!(
            "Generated batch number {} exceeds i32 max",
            masked_value
        ))
    } else {
        Ok(masked_value as i32)
    }
}

#[derive(Hash, Eq, PartialEq)]
pub enum SolanaEvents {
    NativeDeposit,
    SplDeposit,
    NativeWithdrawal,
    SplWithdrawal,
    FinalizeNativeWithdrawal,
    FinalizeSplWithdrawal,
    CommitBatch,
    FinalizeBatch,
    CommitAndFinalizeTransaction,
}

impl SolanaEvents {
    pub fn as_str(&self) -> &'static str {
        match self {
            SolanaEvents::NativeDeposit => "native_deposit",
            SolanaEvents::SplDeposit => "spl_deposit",
            SolanaEvents::NativeWithdrawal => "native_withdrawal",
            SolanaEvents::SplWithdrawal => "spl_withdrawal",
            SolanaEvents::FinalizeNativeWithdrawal => "finalize_native_withdrawal",
            SolanaEvents::FinalizeSplWithdrawal => "finalize_spl_withdrawal",
            SolanaEvents::CommitBatch => "commit_batch",
            SolanaEvents::FinalizeBatch => "finalize_batch",
            SolanaEvents::CommitAndFinalizeTransaction => "commit_and_finalize_transaction",
        }
    }
}
