use crate::entities::{
    l1_deposit, l1_withdraw, l2_withdraw, twine_transaction_batch, twine_transaction_batch_detail,
};
use alloy::rpc::types::Log;
use alloy::sol;
use alloy::sol_types::SolEvent;
use chrono::Utc;
use eyre::Report;
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
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
        uint64 nonce,
        uint64 chainId,
        uint256 blockNumber
    );

    #[derive(Debug)]
    event FinalizeWithdrawERC20(
        string indexed l1Token,
        string indexed l2Token,
        string to,
        string amount,
        uint64 nonce,
        uint64 chainId,
        uint256 blockNumber,
    );
    #[derive(Debug)]
    event CommitBatch(
        uint64 indexed startBlock,
        uint64 indexed endBlock,
        uint256 blockNumber,
        bytes32 indexed batchId,
        bytes32 batchHash
    );

    #[derive(Debug)]
    event FinalizedTransaction(
        uint64 indexed startBlock,
        uint64 indexed endBlock,
        uint64 depositCount,
        uint64 withdrawCount,
        uint256 blockNumber,
        uint64 chainId,
        bytes32 indexed batchId
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
    TwineTransactionBatch(twine_transaction_batch::ActiveModel),
    TwineTransactionBatchDetail(twine_transaction_batch_detail::ActiveModel),
}

#[derive(Debug)]
pub struct ParsedLog {
    pub model: DbModel,
    pub block_number: i64,
}

// FIXME: currently we use the time at which the log was parsed.
// this is buggy as we also reuse this function while syncing missed block later.
// So better approach would be to use the block_timestamp associated with each block
pub async fn parse_log(
    log: Log,
    db: &DatabaseConnection, // Add DatabaseConnection parameter
) -> Result<ParsedLog, Report> {
    let tx_hash = log
        .transaction_hash
        .ok_or(ParserError::MissingTransactionHash)?;

    let tx_hash_str = format!("{tx_hash:?}");

    let block_number = log
        .block_number
        .ok_or_else(|| eyre::eyre!("Missing block number in log"))? as i64;

    let timestamp = log
        .block_timestamp
        .map(|ts| chrono::DateTime::<Utc>::from_timestamp(ts as i64, 0))
        .ok_or_else(|| eyre::eyre!("Missing block timestamp in log"))?
        .ok_or_else(|| eyre::eyre!("Invalid block timestamp"))?;

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
            FinalizeWithdrawERC20::SIGNATURE_HASH => {
                let decoded = log.log_decode::<FinalizeWithdrawERC20>().map_err(|e| {
                    ParserError::DecodeError {
                        event_type: "FinalizeWithdrawERC20",
                        source: Box::new(e),
                    }
                })?;
                let data = decoded.inner.data;
                let model = DbModel::L2Withdraw(l2_withdraw::ActiveModel {
                    chain_id: Set(data.chainId.try_into().unwrap()),
                    nonce: Set(data.nonce.try_into().unwrap()),
                    block_number: Set(Some(data.blockNumber.try_into().unwrap())),
                    slot_number: Set(None),
                    tx_hash: Set(tx_hash_str),
                    l1_token: Set(format!("{:?}", data.l1Token)),
                    l2_token: Set(format!("{:?}", data.l2Token)),
                    from: Set(String::new()),
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
                let model = DbModel::L2Withdraw(l2_withdraw::ActiveModel {
                    chain_id: Set(data.chainId.try_into().unwrap()),
                    nonce: Set(data.nonce.try_into().unwrap()),
                    block_number: Set(Some(data.blockNumber.try_into().unwrap())),
                    slot_number: Set(None),
                    tx_hash: Set(tx_hash_str),
                    l1_token: Set(format!("{:?}", data.l1Token)),
                    l2_token: Set(format!("{:?}", data.l2Token)),
                    from: Set(String::new()),
                    to_twine_address: Set(format!("{:?}", data.to.to_string())),
                    amount: Set(data.amount.to_string()),
                    created_at: Set(Utc::now().into()),
                });
                Ok(ParsedLog {
                    model,
                    block_number,
                })
            }
            CommitBatch::SIGNATURE_HASH => {
                let decoded =
                    log.log_decode::<CommitBatch>()
                        .map_err(|e| ParserError::DecodeError {
                            event_type: "CommitBatch",
                            source: Box::new(e),
                        })?;
                let data = decoded.inner.data;

                // Check if a batch with the same start_block and end_block exists
                let start_block: i64 = data.startBlock.try_into().unwrap();
                let end_block: i64 = data.endBlock.try_into().unwrap();
                let existing_batch = twine_transaction_batch::Entity::find()
                    .filter(twine_transaction_batch::Column::StartBlock.eq(start_block))
                    .filter(twine_transaction_batch::Column::EndBlock.eq(end_block))
                    .one(db)
                    .await
                    .map_err(|e| eyre::eyre!("Failed to query twine_transaction_batch: {:?}", e))?;

                let model = if existing_batch.is_none() {
                    // No existing batch, create a new one
                    DbModel::TwineTransactionBatch(twine_transaction_batch::ActiveModel {
                        start_block: Set(start_block),
                        end_block: Set(end_block),
                        root_hash: Set(data.batchHash.to_string()),
                        timestamp: Set(timestamp.into()),
                        ..Default::default() // number, created_at, updated_at set by DB
                    })
                } else {
                    // Batch exists, no need to insert a new one
                    // We could return a different DbModel variant or handle this in the caller
                    // For now, we'll return an empty model to avoid insertion
                    DbModel::TwineTransactionBatch(twine_transaction_batch::ActiveModel {
                        ..Default::default()
                    })
                };

                Ok(ParsedLog {
                    model,
                    block_number,
                })
            }
            FinalizedTransaction::SIGNATURE_HASH => {
                let decoded = log.log_decode::<FinalizedTransaction>().map_err(|e| {
                    ParserError::DecodeError {
                        event_type: "FinalizedTransaction",
                        source: Box::new(e),
                    }
                })?;
                let data = decoded.inner.data;

                // Find the batch with matching start_block and end_block
                let start_block: i64 = data.startBlock.try_into().unwrap();
                let end_block: i64 = data.endBlock.try_into().unwrap();
                let existing_batch = twine_transaction_batch::Entity::find()
                    .filter(twine_transaction_batch::Column::StartBlock.eq(start_block))
                    .filter(twine_transaction_batch::Column::EndBlock.eq(end_block))
                    .one(db)
                    .await
                    .map_err(|e| eyre::eyre!("Failed to query twine_transaction_batch: {:?}", e))?;

                let batch_number = match existing_batch {
                    Some(batch) => batch.number,
                    None => {
                        // Ideally, a FinalizedTransaction should always have a corresponding CommitBatch
                        // If no batch exists, you might want to handle this as an error or log a warning
                        tracing::warn!(
                            "No batch found for FinalizedTransaction with start_block = {}, end_block = {}",
                            start_block,
                            end_block
                        );
                        return Err(eyre::eyre!(
                            "No matching batch found for FinalizedTransaction"
                        ));
                    }
                };

                let model = DbModel::TwineTransactionBatchDetail(
                    twine_transaction_batch_detail::ActiveModel {
                        batch_number: Set(batch_number.into()),

                        l2_transaction_count: Set(data.depositCount.try_into().unwrap()), //just a placeholder

                        l2_fair_gas_price: Set((0.0).try_into().unwrap()), // Placeholder
                        chain_id: Set(data.chainId.try_into().unwrap()),
                        ..Default::default() // commit_id, execute_id, created_at, updated_at set by DB
                    },
                );

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
