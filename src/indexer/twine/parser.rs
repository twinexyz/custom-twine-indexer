use crate::entities::{twine_l1_deposit, twine_l1_withdraw, twine_transaction_batch};
use alloy::rpc::types::Log;
use alloy::sol_types::{SolEvent, SolType};
use chrono::{DateTime, Utc};
use eyre::Report;
use sea_orm::ActiveValue;
use twine_evm_contracts::evm::ethereum::twine_chain::TwineChain;
use twine_evm_contracts::evm::twine::l2_messenger::{L2Messenger, PrecompileReturn};

#[derive(Debug)]
pub enum ParserError {
    MissingTransactionHash,
    DecodeError {
        event_type: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    UnknownEvent {
        signature: alloy::primitives::B256,
    },
}

impl std::error::Error for ParserError {}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserError::MissingTransactionHash => write!(f, "Missing transaction hash in log"),
            ParserError::DecodeError { event_type, source } => {
                write!(f, "Failed to decode {} event: {}", event_type, source)
            }
            ParserError::UnknownEvent { signature } => {
                write!(f, "Unknown event type: {}", signature)
            }
        }
    }
}

#[derive(Debug)]
pub enum DbModel {
    TwineL1Deposit(twine_l1_deposit::ActiveModel),
    TwineL1Withdraw(twine_l1_withdraw::ActiveModel),
    TwineTransactionBatch(twine_transaction_batch::ActiveModel),
}

#[derive(Debug)]
pub struct ParsedLog {
    pub model: DbModel,
    pub block_number: i64,
}

fn process_precompile_return(
    pr: PrecompileReturn,
    tx_hash: alloy::primitives::B256,
    block_number: i64,
) -> Result<ParsedLog, Report> {
    if let Some(deposit_txn) = pr.deposit.first() {
        Ok(ParsedLog {
            model: DbModel::TwineL1Deposit(twine_l1_deposit::ActiveModel {
                l1_nonce: ActiveValue::Set(deposit_txn.l1_nonce as i64),
                chain_id: ActiveValue::Set(deposit_txn.detail.chain_id as i64),
                status: ActiveValue::Set(deposit_txn.detail.status as i16),
                slot_number: ActiveValue::Set(deposit_txn.detail.slot_number as i64),
                from_address: ActiveValue::Set(deposit_txn.detail.from_address.clone()),
                to_twine_address: ActiveValue::Set(deposit_txn.detail.to_twine_address.clone()),
                l1_token: ActiveValue::Set(deposit_txn.detail.l1_token.clone()),
                l2_token: ActiveValue::Set(deposit_txn.detail.l2_token.clone()),
                amount: ActiveValue::Set(deposit_txn.detail.amount.clone()),
                tx_hash: ActiveValue::Set(tx_hash.to_string()),
            }),
            block_number,
        })
    } else if let Some(withdraw_txn) = pr.withdraws.first() {
        Ok(ParsedLog {
            model: DbModel::TwineL1Withdraw(twine_l1_withdraw::ActiveModel {
                l1_nonce: ActiveValue::Set(withdraw_txn.l1_nonce as i64),
                chain_id: ActiveValue::Set(withdraw_txn.detail.chain_id as i64),
                status: ActiveValue::Set(withdraw_txn.detail.status as i16),
                slot_number: ActiveValue::Set(withdraw_txn.detail.slot_number as i64),
                from_address: ActiveValue::Set(withdraw_txn.detail.from_address.clone()),
                to_twine_address: ActiveValue::Set(withdraw_txn.detail.to_twine_address.clone()),
                l1_token: ActiveValue::Set(withdraw_txn.detail.l1_token.clone()),
                l2_token: ActiveValue::Set(withdraw_txn.detail.l2_token.clone()),
                amount: ActiveValue::Set(withdraw_txn.detail.amount.clone()),
                tx_hash: ActiveValue::Set(tx_hash.to_string()),
            }),
            block_number,
        })
    } else {
        Err(eyre::eyre!("No deposit or withdraw events found"))
    }
}

pub fn parse_log(log: Log) -> Result<ParsedLog, Report> {
    // Unwrap the transaction hash and block number.
    let tx_hash = log
        .transaction_hash
        .ok_or_else(|| eyre::eyre!("Missing tx_hash in log"))?;
    let block_number = log
        .block_number
        .ok_or_else(|| eyre::eyre!("Missing block number in log"))? as i64;

    // Get the block timestamp from the log
    let timestamp = log
        .block_timestamp
        .map(|ts| chrono::DateTime::<Utc>::from_timestamp(ts as i64, 0))
        .ok_or_else(|| eyre::eyre!("Missing block timestamp in log"))?
        .ok_or_else(|| eyre::eyre!("Invalid block timestamp"))?;

    let topic = log
        .topic0()
        .ok_or_else(|| eyre::eyre!("Missing topic0 in log"))?;

    match *topic {
        L2Messenger::EthereumTransactionsHandled::SIGNATURE_HASH => {
            let decoded = log.log_decode::<L2Messenger::EthereumTransactionsHandled>()?;
            let event = decoded.inner.data.transactionOutput;
            let pr = PrecompileReturn::abi_decode(&event, true)
                .map_err(|e| eyre::eyre!("ABI decode error: {}", e))?;
            process_precompile_return(pr, tx_hash, block_number)
        }
        L2Messenger::SolanaTransactionsHandled::SIGNATURE_HASH => {
            let decoded = log.log_decode::<L2Messenger::SolanaTransactionsHandled>()?;
            let event = decoded.inner.data.transactionOutput;
            let pr = PrecompileReturn::abi_decode(&event, true)
                .map_err(|e| eyre::eyre!("ABI decode error: {}", e))?;
            process_precompile_return(pr, tx_hash, block_number)
        }
        TwineChain::CommitBatch::SIGNATURE_HASH => {
            let decoded = log.log_decode::<TwineChain::CommitBatch>()?;
            let data = decoded.inner.data;

            let model = DbModel::TwineTransactionBatch(twine_transaction_batch::ActiveModel {
                number: ActiveValue::Set(
                    data.blockNumber
                        .try_into()
                        .map_err(|e| eyre::eyre!("Failed to convert blockNumber to i64: {}", e))?,
                ),
                timestamp: ActiveValue::Set(timestamp.into()),
                start_block: ActiveValue::Set(data.startBlock as i64),
                end_block: ActiveValue::Set(data.endBlock as i64),
                root_hash: ActiveValue::Set(data.batchHash.0.to_vec()),
                created_at: ActiveValue::Set(timestamp.into()),
                updated_at: ActiveValue::Set(timestamp.into()),
            });
            Ok(ParsedLog {
                model,
                block_number,
            })
        }
        _ => Err(eyre::eyre!("Unknown event signature: {:?}", topic)),
    }
}
