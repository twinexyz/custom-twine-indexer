use crate::entities::{l1_deposit, l1_withdraw, sent_message};
use alloy::rpc::types::Log;
use alloy::sol_types::SolEvent;
use chrono::Utc;
use eyre::Report;
use sea_orm::ActiveValue::Set;
use twine_evm_contracts::evm::{
    ethereum::l1_message_queue::L1MessageQueue, twine::l2_messenger::L2Messenger,
};

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
    SentMessage(sent_message::ActiveModel),
    L1Deposit(l1_deposit::ActiveModel),
    L1Withdraw(l1_withdraw::ActiveModel),
}

#[derive(Debug)]
pub struct ParsedLog {
    pub model: DbModel,
    pub block_number: i64,
}

// FIXME: currently we use the time at which the log was parsed.
// this is buggy as we also reuse this function while syncing missed block later.
// So better approach would be to use the block_timestamp associated with each block
pub fn parse_log(log: Log) -> Result<ParsedLog, Report> {
    let tx_hash = log
        .transaction_hash
        .ok_or(ParserError::MissingTransactionHash)?;

    let tx_hash_str = format!("{tx_hash:?}");

    let block_number = log
        .block_number
        .ok_or_else(|| eyre::eyre!("Missing block number in log"))? as i64;

    match log.topic0() {
        Some(sig) => match *sig {
            L2Messenger::SentMessage::SIGNATURE_HASH => {
                let decoded = log.log_decode::<L2Messenger::SentMessage>().map_err(|e| {
                    ParserError::DecodeError {
                        event_type: "SentMessage",
                        source: Box::new(e),
                    }
                })?;

                let data = decoded.inner.data;
                let model = DbModel::SentMessage(sent_message::ActiveModel {
                    tx_hash: Set(tx_hash_str),
                    from: Set(format!("{:?}", data.from)),
                    l2_token: Set(format!("{:?}", data.l2Token)),
                    to: Set(format!("{:?}", data.to)),
                    l1_token: Set(format!("{:?}", data.l1Token)),
                    amount: Set(data.amount.to_string()),
                    nonce: Set(data.nonce.try_into().unwrap()),
                    chain_id: Set(data.chainId.try_into().unwrap()),
                    block_number: Set(data.blockNumber.try_into().unwrap()),
                    gas_limit: Set(data.gasLimit.try_into().unwrap()),
                    created_at: Set(Utc::now().into()),
                });
                Ok(ParsedLog {
                    model,
                    block_number,
                })
            }
            L1MessageQueue::QueueDepositTransaction::SIGNATURE_HASH => {
                let decoded = log
                    .log_decode::<L1MessageQueue::QueueDepositTransaction>()
                    .map_err(|e| ParserError::DecodeError {
                        event_type: "QueueDepositTransaction",
                        source: Box::new(e),
                    })?;

                let data = decoded.inner.data;
                let model = DbModel::L1Deposit(l1_deposit::ActiveModel {
                    tx_hash: Set(tx_hash_str),
                    nonce: Set(data.nonce.try_into().unwrap()),
                    chain_id: Set(data.chainId.try_into().unwrap()),
                    block_number: Set(data.block_number.try_into().unwrap()),
                    l1_token: Set(format!("{:?}", data.l1_token)),
                    l2_token: Set(format!("{:?}", data.l2_token)),
                    from: Set(format!("{:?}", data.from)),
                    to_twine_address: Set(format!("{:?}", data.to_twine_address)),
                    amount: Set(data.amount.to_string()),
                    created_at: Set(Utc::now().into()),
                });
                Ok(ParsedLog {
                    model,
                    block_number,
                })
            }
            L1MessageQueue::QueueWithdrawalTransaction::SIGNATURE_HASH => {
                let decoded = log
                    .log_decode::<L1MessageQueue::QueueWithdrawalTransaction>()
                    .map_err(|e| ParserError::DecodeError {
                        event_type: "QueueWithdrawalTransaction",
                        source: Box::new(e),
                    })?;

                let data = decoded.inner.data;
                let model = DbModel::L1Withdraw(l1_withdraw::ActiveModel {
                    tx_hash: Set(tx_hash_str),
                    nonce: Set(data.nonce.try_into().unwrap()),
                    chain_id: Set(data.chainId.try_into().unwrap()),
                    block_number: Set(data.block_number.try_into().unwrap()),
                    l1_token: Set(format!("{:?}", data.l1_token)),
                    l2_token: Set(format!("{:?}", data.l2_token)),
                    from: Set(format!("{:?}", data.from)),
                    to_twine_address: Set(format!("{:?}", data.to_twine_address)),
                    amount: Set(data.amount.to_string()),
                    created_at: Set(Utc::now().into()),
                });
                Ok(ParsedLog {
                    model,
                    block_number,
                })
            }
            other => Err(ParserError::UnknownEvent { signature: other }.into()),
        },
        None => Err(ParserError::UnknownEvent {
            signature: alloy::primitives::B256::ZERO,
        }
        .into()),
    }
}
