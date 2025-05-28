use base64::{engine::general_purpose, Engine as _};
use borsh::{BorshDeserialize, BorshSerialize};
use chrono::{DateTime, Utc};
use eyre::Result;
use num_traits::FromPrimitive;
use sea_orm::prelude::Decimal;
use sea_orm::{
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use solana_sdk::native_token::Sol;
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

#[derive(Hash, Eq, PartialEq, Debug, Serialize, Clone)]
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

pub struct LogMetadata {
    pub event_type: SolanaEvents,
    pub encoded_data: String,
}

pub fn extract_log(logs: &[String]) -> Option<LogMetadata> {
    debug!("Parsing logs: {:?}", logs);

    let mut event_type = None;
    let mut encoded_data = None;

    for log in logs {
        match log.as_str() {
            "Program log: Instruction: NativeTokenDeposit" => {
                event_type = Some(SolanaEvents::NativeDeposit)
            }
            "Program log: Instruction: SplTokensDeposit" => {
                event_type = Some(SolanaEvents::SplDeposit)
            }
            "Program log: Instruction: ForcedNativeTokenWithdrawal" => {
                event_type = Some(SolanaEvents::NativeWithdrawal)
            }
            "Program log: Instruction: ForcedSplTokenWithdrawal" => {
                event_type = Some(SolanaEvents::SplWithdrawal)
            }
            "Program log: Instruction: FinalizeNativeWithdrawal" => {
                event_type = Some(SolanaEvents::FinalizeNativeWithdrawal)
            }
            "Program log: Instruction: FinalizeSplWithdrawal" => {
                event_type = Some(SolanaEvents::FinalizeSplWithdrawal)
            }
            "Program log: Instruction: CommitBatch" => event_type = Some(SolanaEvents::CommitBatch),
            "Program log: Instruction: FinalizeBatch" => {
                event_type = Some(SolanaEvents::FinalizeBatch)
            }
            "Program log: Instruction: CommitAndFinalizeTransaction" => {
                event_type = Some(SolanaEvents::CommitAndFinalizeTransaction)
            }
            log if log.starts_with("Program data: ") => {
                encoded_data = Some(log.trim_start_matches("Program data: ").to_string());
            }
            _ => continue,
        }
    }

    let Some(event_type) = event_type else {
        debug!("No recognized event type found in logs: {:?}", logs);
        return None;
    };
    let Some(encoded_data) = encoded_data else {
        debug!(
            "No encoded data found for event {} in logs: {:?}",
            event_type.as_str(),
            logs
        );
        return None;
    };

    debug!(
        "Identified event_type: {}, encoded_data: {}",
        event_type.as_str(),
        encoded_data
    );

    Some(LogMetadata {
        event_type,
        encoded_data,
    })
}

#[derive(Debug, Serialize, Clone)] // Added Clone for convenience
pub struct FoundEvent {
    pub event_type: SolanaEvents, // Storing as string representation of the enum
    pub transaction_signature: String,
    pub slot: u64,
    pub timestamp: DateTime<Utc>,
    pub encoded_data: String,
}

impl FoundEvent {
    pub fn parse_borsh<T: BorshDeserialize>(&self) -> eyre::Result<T> {
        let encoded_data = &self.encoded_data;

        debug!("Parsing Borsh data: encoded_data = {}", encoded_data);

        let decoded_data = general_purpose::STANDARD
            .decode(encoded_data)
            .map_err(|e| eyre::eyre!("Failed to decode base64: {}", e))?;

        let data_with_discriminator = if decoded_data.len() >= 8 {
            &decoded_data[8..]
        } else {
            &decoded_data[..]
        };

        let mut event = match T::try_from_slice(data_with_discriminator) {
            Ok(event) => event,
            Err(_) => T::try_from_slice(&decoded_data)
                .map_err(|e| eyre::eyre!("Failed to deserialize Borsh data: {}", e))?,
        };

        Ok(event)
    }
}
