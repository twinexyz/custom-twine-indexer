use crate::entities::{l1_deposit, l1_withdraw, l2_withdraw};
use alloy::rpc::types::Log;
use alloy::sol;
use alloy::sol_types::SolEvent;
use chrono::Utc;
use eyre::Report;
use sea_orm::ActiveValue::Set;
use twine_evm_contracts::evm::ethereum::l1_message_queue::L1MessageQueue;

// TODO: ask these signatures to be added to
// contracts-metadata.
// So that we can use the import as
// use twine_evm_contracts::evm::ethereum::gateways::{
//  erc20::FinalizeWithdrawETH,
//  eth::FinalizeWithdrawETH
// };

sol! {
    #[derive(Debug)]
    event FinalizeWithdrawETH(
        string l1Token,
        string l2Token,
        string indexed to,
        string amount,
        uint256 blockNumber
    );

    #[derive(Debug)]
    event FinalizeWithdrawERC20(
        string indexed l1Token,
        string indexed l2Token,
        string to,
        string amount,
        uint256 blockNumber
    );
}

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
    L1Deposit(l1_deposit::ActiveModel),
    L1Withdraw(l1_withdraw::ActiveModel),
    L2Withdraw(l2_withdraw::ActiveModel),
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
            L1MessageQueue::QueueDepositTransaction::SIGNATURE_HASH => {
                let decoded = log
                    .log_decode::<L1MessageQueue::QueueDepositTransaction>()
                    .map_err(|e| ParserError::DecodeError {
                        event_type: "QueueDepositTransaction",
                        source: Box::new(e),
                    })?;

                let data = decoded.inner.data;
                let model = DbModel::L1Deposit(l1_deposit::ActiveModel {
                    nonce: Set(data.nonce.try_into().unwrap()),
                    chain_id: Set(data.chainId.try_into().unwrap()),
                    block_number: Set(Some(data.blockNumber.try_into().unwrap())),
                    slot_number: Set(None),
                    from: Set(format!("{:?}", data.from)),
                    to_twine_address: Set(format!("{:?}", data.toTwineAddress)),
                    l1_token: Set(format!("{:?}", data.l1Token)),
                    l2_token: Set(format!("{:?}", data.l2Token)),
                    tx_hash: Set(tx_hash_str),
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
                    block_number: Set(Some(data.blockNumber.try_into().unwrap())),
                    slot_number: Set(None),
                    l1_token: Set(format!("{:?}", data.l1Token)),
                    l2_token: Set(format!("{:?}", data.l2Token)),
                    from: Set(format!("{:?}", data.from)),
                    to_twine_address: Set(format!("{:?}", data.toTwineAddress)),
                    amount: Set(data.amount.to_string()),
                    created_at: Set(Utc::now().into()),
                });
                Ok(ParsedLog {
                    model,
                    block_number,
                })
            }
            // waiting for somes changes in smart contract
            FinalizeWithdrawERC20::SIGNATURE_HASH => {
                let decoded = log.log_decode::<FinalizeWithdrawERC20>().map_err(|e| {
                    ParserError::DecodeError {
                        event_type: "FinalizeWithdrawERC20",
                        source: Box::new(e),
                    }
                })?;
                let data = decoded.inner.data;
                let model = DbModel::L2Withdraw(l2_withdraw::ActiveModel {
                    // chain_id: Set(data.chainId.try_into().unwrap()),
                    // nonce: Set(data.nonce.try_into().unwrap()),
                    block_number: Set(Some(data.blockNumber.try_into().unwrap())),
                    slot_number: Set(None),
                    // tx_hash: Set(tx_hash_str),
                    l1_token: Set(format!("{:?}", data.l1Token)),
                    l2_token: Set(format!("{:?}", data.l2Token)),
                    // from: Set(format!("{:?}", data.from)),
                    to_twine_address: Set(format!("{:?}", data.to.to_string())),
                    amount: Set(data.amount.to_string()),
                    created_at: Set(Utc::now().into()),
                });
                Ok(ParsedLog {
                    model,
                    block_number,
                })
            }
            FinalizeWithdrawETH::SIGNATURE_HASH => {
                let decoded = log.log_decode::<FinalizeWithdrawETH>().map_err(|e| {
                    ParserError::DecodeError {
                        event_type: "FinalizeWithdrawETH",
                        source: Box::new(e),
                    }
                })?;
                let data = decoded.inner.data;
                // waiting for this change in smart contract
                let model = DbModel::L2Withdraw(l2_withdraw::ActiveModel {
                    // chain_id: Set(data.chainId.try_into().unwrap()),
                    // nonce: Set(data.nonce.try_into().unwrap()),
                    block_number: Set(Some(data.blockNumber.try_into().unwrap())),
                    slot_number: Set(None),
                    // tx_hash: Set(tx_hash_str),
                    l1_token: Set(format!("{:?}", data.l1Token)),
                    l2_token: Set(format!("{:?}", data.l2Token)),
                    // from: Set(format!("{:?}", data.from)),
                    to_twine_address: Set(format!("{:?}", data.to.to_string())),
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
