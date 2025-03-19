use crate::entities::{
    twine_l1_deposit, twine_l1_withdraw, twine_l2_withdraw, twine_transaction_batch,
};
use alloy::rpc::types::Log;
use alloy::sol_types::{SolEvent, SolType};
use chrono::Utc;
use eyre::Report;

use sea_orm::ActiveValue::Set;
use twine_evm_contracts::evm::twine::l2_messenger::{L2Messenger, PrecompileReturn};

#[derive(Debug)]
pub enum ParserError {
    MissingTransactionHash,
    DecodeError {
        event_type: &'static str,
        source: Report,
    },
    UnknownEvent {
        signature: alloy::primitives::B256,
    },
    SkipLog,
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
            ParserError::SkipLog => {
                write!(f, "SkipLog")
            }
        }
    }
}

#[derive(Debug)]
pub enum DbModel {
    TwineL1Deposit(twine_l1_deposit::ActiveModel),
    TwineL1Withdraw(twine_l1_withdraw::ActiveModel),
    TwineL2Withdraw(twine_l2_withdraw::ActiveModel),
    // TwineTransactionBatch(twine_transaction_batch::ActiveModel),
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
) -> Option<ParsedLog> {
    pr.deposit
        .first()
        .map(|deposit_txn| ParsedLog {
            model: DbModel::TwineL1Deposit(twine_l1_deposit::ActiveModel {
                l1_nonce: Set(deposit_txn.l1_nonce as i64),
                chain_id: Set(deposit_txn.detail.chain_id as i64),
                status: Set(deposit_txn.detail.status as i16),
                slot_number: Set(deposit_txn.detail.slot_number as i64),
                from_address: Set(deposit_txn.detail.from_address.clone()),
                to_twine_address: Set(deposit_txn.detail.to_twine_address.clone()),
                l1_token: Set(deposit_txn.detail.l1_token.clone()),
                l2_token: Set(deposit_txn.detail.l2_token.clone()),
                amount: Set(deposit_txn.detail.amount.clone()),
                tx_hash: Set(tx_hash.to_string()),
            }),
            block_number,
        })
        .or_else(|| {
            pr.withdraws.first().map(|withdraw_txn| ParsedLog {
                model: DbModel::TwineL1Withdraw(twine_l1_withdraw::ActiveModel {
                    l1_nonce: Set(withdraw_txn.l1_nonce as i64),
                    chain_id: Set(withdraw_txn.detail.chain_id as i64),
                    status: Set(withdraw_txn.detail.status as i16),
                    slot_number: Set(withdraw_txn.detail.slot_number as i64),
                    from_address: Set(withdraw_txn.detail.from_address.clone()),
                    to_twine_address: Set(withdraw_txn.detail.to_twine_address.clone()),
                    l1_token: Set(withdraw_txn.detail.l1_token.clone()),
                    l2_token: Set(withdraw_txn.detail.l2_token.clone()),
                    amount: Set(withdraw_txn.detail.amount.clone()),
                    tx_hash: Set(tx_hash.to_string()),
                }),
                block_number,
            })
        })
}

pub fn parse_log(log: Log) -> Result<ParsedLog, Report> {
    let tx_hash = log
        .transaction_hash
        .ok_or_else(|| eyre::eyre!("Missing tx_hash in log"))?;
    let block_number = log
        .block_number
        .ok_or_else(|| eyre::eyre!("Missing block number in log"))? as i64;

    let timestamp = log
        .block_timestamp
        .and_then(|ts| chrono::DateTime::<Utc>::from_timestamp(ts as i64, 0))
        .unwrap_or_else(|| {
            tracing::warn!("Missing or invalid block timestamp in log. Using default timestamp.");
            Utc::now()
        });

    let topic = log
        .topic0()
        .ok_or_else(|| eyre::eyre!("Missing topic0 in log"))?;

    match *topic {
        L2Messenger::EthereumTransactionsHandled::SIGNATURE_HASH => {
            let decoded = log.log_decode::<L2Messenger::EthereumTransactionsHandled>()?;
            let event = decoded.inner.data.transactionOutput;
            let pr = PrecompileReturn::abi_decode(&event, true)
                .map_err(|e| eyre::eyre!("ABI decode error: {}", e))?;
            process_precompile_return(pr, tx_hash, block_number).ok_or(ParserError::SkipLog.into())
        }
        L2Messenger::SolanaTransactionsHandled::SIGNATURE_HASH => {
            let decoded = log.log_decode::<L2Messenger::SolanaTransactionsHandled>()?;
            let event = decoded.inner.data.transactionOutput;
            let pr = PrecompileReturn::abi_decode(&event, true)
                .map_err(|e| eyre::eyre!("ABI decode error: {}", e))?;
            process_precompile_return(pr, tx_hash, block_number).ok_or(ParserError::SkipLog.into())
        }
        L2Messenger::SentMessage::SIGNATURE_HASH => {
            let decoded = log.log_decode::<L2Messenger::SentMessage>()?;
            let data = decoded.inner.data;
            let model = DbModel::TwineL2Withdraw(twine_l2_withdraw::ActiveModel {
                from: Set(format!("{:?}", data.from)),
                l2_token: Set(format!("{:?}", data.l2Token)),
                to: Set(format!("{:?}", data.to)),
                l1_token: Set(format!("{:?}", data.l1Token)),
                amount: Set(data.amount.to_string()),
                nonce: Set(data.nonce.to_string()),
                value: Set(data.value.to_string()),
                chain_id: Set(data.chainId.to_string()),
                block_number: Set(data.blockNumber.to_string()),
                gas_limit: Set(data.gasLimit.to_string()),
                tx_hash: Set(tx_hash.to_string()),
            });
            Ok(ParsedLog {
                model,
                block_number,
            })
        }
        other => Err(ParserError::UnknownEvent { signature: other }.into()),
    }
}
