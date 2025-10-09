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
use serde_json::Value;
use solana_client::rpc_response::{Response, RpcLogsResponse};
use solana_sdk::native_token::Sol;
use std::env;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info};

#[derive(Debug, Clone, Deserialize)]
pub struct SolanaLog {
    pub event: SolanaEvent,
    pub timestamp: DateTime<Utc>,
    pub slot_number: u64,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MessageTransactionEvent {
    pub event: String,
    pub nonce: u64,
    pub l1_pubkey: String,
    pub twine_address: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: String,
    pub data: Vec<u8>,
    pub message_type: String,
    pub slot_number: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommitBatchEvent {
    pub event: String,
    pub batch_number: u64,
    pub batch_hash: [u8; 32],
    pub chain_id: u64,
    pub slot_number: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FinalizeBatchEvent {
    pub event: String,
    pub batch_number: u64,
    pub batch_hash: [u8; 32],
    pub chain_id: u64,
    pub slot_number: u64,
    pub messages_handled_on_twine: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BatchCommitmentAndFinalizationSuccessfulEvent {
    pub event: String,
    pub batch_number: u64,
    pub batch_hash: [u8; 32],
    pub chain_id: u64,
    pub slot_number: u64,
    pub messages_handled_on_twine: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RefundSuccessfulEvent {
    pub event: String,
    pub nonce: u64,
    pub l1_receiver: String,
    pub l1_token: String,
    pub chain_id: u64,
    pub amount: u64,
    pub slot_number: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForcedWithdrawalSuccessfulEvent {
    pub event: String,
    pub nonce: u64,
    pub l1_receiver: String,
    pub l1_token: String,
    pub chain_id: u64,
    pub amount: u64,
    pub slot_number: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct L2WithdrawExecutedEvent {
    pub event: String,
    pub nonce: u64,
    pub l1_receiver: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: u64,
    pub slot_number: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub enum SolanaEvent {
    MessageTransaction(MessageTransactionEvent),
    CommitBatch(CommitBatchEvent),
    FinalizeBatch(FinalizeBatchEvent),
    BatchCommitmentAndFinalizationSuccessful(BatchCommitmentAndFinalizationSuccessfulEvent),
    RefundSuccessful(RefundSuccessfulEvent),
    ForcedWithdrawalSuccessful(ForcedWithdrawalSuccessfulEvent),
    L2WithdrawExecuted(L2WithdrawExecutedEvent),
    Unknown(Value), // For events we don't have specific structs for
}

/// Parse a JSON log string by first extracting the event type and then deserializing appropriately
pub fn parse_json_log(log: &str) -> Result<SolanaEvent> {
    let actual_log = log.trim_start_matches("Program log: ");

    // First, parse as generic JSON to extract the event field
    let json_value: Value =
        serde_json::from_str(actual_log).map_err(|e| eyre::eyre!("Failed to parse JSON: {}", e))?;

    // Extract the event field
    let event_type = json_value
        .get("event")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("No 'event' field found in JSON"))?;

    info!("Parsing event type: {}", event_type);
    match event_type {
        "MessageTransaction" => {
            let event: MessageTransactionEvent = serde_json::from_str(actual_log)
                .map_err(|e| eyre::eyre!("Failed to deserialize MessageTransaction: {}", e))?;
            Ok(SolanaEvent::MessageTransaction(event))
        }
        "CommitedBatch" => {
            let event: CommitBatchEvent = serde_json::from_str(actual_log)
                .map_err(|e| eyre::eyre!("Failed to deserialize Batch: {}", e))?;
            Ok(SolanaEvent::CommitBatch(event))
        }
        "FinalizedBatch" => {
            let event: FinalizeBatchEvent = serde_json::from_str(actual_log)
                .map_err(|e| eyre::eyre!("Failed to deserialize Batch: {}", e))?;
            Ok(SolanaEvent::FinalizeBatch(event))
        }
        "BatchCommitmentAndFinalizationSuccessful" => {
            let event: BatchCommitmentAndFinalizationSuccessfulEvent =
                serde_json::from_str(actual_log)
                    .map_err(|e| eyre::eyre!("Failed to deserialize Batch: {}", e))?;
            Ok(SolanaEvent::BatchCommitmentAndFinalizationSuccessful(event))
        }
        "RefundSuccessful" => {
            let event: RefundSuccessfulEvent = serde_json::from_str(actual_log)
                .map_err(|e| eyre::eyre!("Failed to deserialize Batch: {}", e))?;
            Ok(SolanaEvent::RefundSuccessful(event))
        }
        "ForcedWithdrawalSuccessful" => {
            let event: ForcedWithdrawalSuccessfulEvent = serde_json::from_str(actual_log)
                .map_err(|e| eyre::eyre!("Failed to deserialize Batch: {}", e))?;
            Ok(SolanaEvent::ForcedWithdrawalSuccessful(event))
        }
        "L2WithdrawExecuted" => {
            let event: L2WithdrawExecutedEvent = serde_json::from_str(actual_log)
                .map_err(|e| eyre::eyre!("Failed to deserialize Batch: {}", e))?;
            Ok(SolanaEvent::L2WithdrawExecuted(event))
        }
        _ => {
            debug!(
                "Unknown event type: {}, storing as generic JSON",
                event_type
            );
            Ok(SolanaEvent::Unknown(json_value))
        }
    }
}

impl SolanaEvent {
    pub fn get_event_type(&self) -> &str {
        match self {
            SolanaEvent::MessageTransaction(event) => &event.event,
            SolanaEvent::CommitBatch(event) => &event.event,
            SolanaEvent::FinalizeBatch(event) => &event.event,
            SolanaEvent::BatchCommitmentAndFinalizationSuccessful(event) => &event.event,
            SolanaEvent::RefundSuccessful(event) => &event.event,
            SolanaEvent::ForcedWithdrawalSuccessful(event) => &event.event,
            SolanaEvent::L2WithdrawExecuted(event) => &event.event,
            SolanaEvent::Unknown(event) => "Unknown",
        }
    }
}

pub fn parse_log(response: Response<RpcLogsResponse>) -> eyre::Result<SolanaLog> {
    let signature = response.value.signature;
    let logs = response.value.logs;
    let slot = response.context.slot;

    for log in logs {
        let event = parse_json_log(&log);

        if let Ok(event) = event {
            return Ok(SolanaLog {
                event: event,
                signature: signature,
                slot_number: slot,
                timestamp: Utc::now(), // Live events use current time
            });
        }
    }
    Err(eyre::eyre!("No relevant events found"))
}
