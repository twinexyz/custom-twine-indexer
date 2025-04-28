use std::env;

use super::{FinalizeWithdrawERC20, FinalizeWithdrawETH};
use alloy::primitives::B256;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::{BlockTransactions, Log};
use alloy::sol;
use alloy::sol_types::SolEvent;
use blake3::hash;
use chrono::{DateTime, Utc};
use common::blockscout_entities::{
    twine_batch_l2_blocks, twine_batch_l2_transactions, twine_lifecycle_l1_transactions,
    twine_transaction_batch, twine_transaction_batch_detail,
};
use common::entities::{l1_deposit, l1_withdraw, l2_withdraw};
use eyre::Report;
use num_traits::FromPrimitive;
use sea_orm::prelude::Decimal;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tracing::{debug, error, info};
use twine_evm_contracts::evm::ethereum::l1_message_queue::L1MessageQueue;
use twine_evm_contracts::evm::ethereum::twine_chain::TwineChain::{
    CommitBatch, FinalizedBatch, FinalizedTransaction,
};

#[derive(Debug)]
pub enum ParserError {
    MissingTransactionHash,
    MissingBlockNumber,
    MissingBlockTimestamp,
    InvalidBlockTimestamp,
    DecodeError {
        event_type: &'static str,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    UnknownEvent {
        signature: alloy::primitives::B256,
    },
    BatchNotFound {
        start_block: u64,
        end_block: u64,
    },
    FinalizedBeforeCommit {
        start_block: u64,
        end_block: u64,
        batch_hash: String,
    },
    NumberOverflow {
        value: u64,
    },
}

impl std::error::Error for ParserError {}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParserError::MissingTransactionHash => write!(f, "Missing transaction hash in log"),
            ParserError::MissingBlockNumber => write!(f, "Missing block number in log"),
            ParserError::MissingBlockTimestamp => write!(f, "Missing block timestamp in log"),
            ParserError::InvalidBlockTimestamp => write!(f, "Invalid block timestamp"),
            ParserError::DecodeError { event_type, source } => {
                write!(f, "Failed to decode {} event: {}", event_type, source)
            }
            ParserError::UnknownEvent { signature } => {
                write!(f, "Unknown event type: {}", signature)
            }
            ParserError::BatchNotFound { start_block, end_block } => write!(
                f,
                "Batch not found for start_block: {}, end_block: {}",
                start_block, end_block
            ),
            ParserError::FinalizedBeforeCommit {
                start_block,
                end_block,
                batch_hash,
            } => write!(
                f,
                "Finalized event indexed before commit for start_block: {}, end_block: {}, batch_hash: {}",
                start_block, end_block, batch_hash
            ),
            ParserError::NumberOverflow { value } => write!(
                f,
                "Generated batch number {} exceeds i32 maximum value",
                value
            ),
        }
    }
}

#[derive(Debug)]
pub enum DbModel {
    L1Deposit(l1_deposit::ActiveModel),
    L1Withdraw(l1_withdraw::ActiveModel),
    L2Withdraw(l2_withdraw::ActiveModel),
    TwineTransactionBatch {
        model: twine_transaction_batch::ActiveModel,
        chain_id: Decimal,
        tx_hash: String,
        l2_blocks: Vec<twine_batch_l2_blocks::ActiveModel>,
        l2_transactions: Vec<twine_batch_l2_transactions::ActiveModel>,
    },
    TwineTransactionBatchDetail(twine_transaction_batch_detail::ActiveModel),
    TwineLifecycleL1Transactions {
        model: twine_lifecycle_l1_transactions::ActiveModel,
        batch_number: i32,
    },
}

#[derive(Debug)]
pub struct ParsedLog {
    pub model: DbModel,
    pub block_number: i64,
}

fn generate_batch_number(start_block: u64, end_block: u64) -> Result<i32, ParserError> {
    debug!(
        "Generating batch number for start_block: {}, end_block: {}",
        start_block, end_block
    );
    let input = format!("{}:{}", start_block, end_block);
    let digest = hash(input.as_bytes());
    let value = u64::from_le_bytes(digest.as_bytes()[..8].try_into().unwrap());
    let masked_value = value & 0x7FFF_FFFF; // Ensures it fits in i32 positive range
    if masked_value > i32::MAX as u64 {
        error!("Batch number generation overflow: value={}", masked_value);
        Err(ParserError::NumberOverflow {
            value: masked_value,
        })
    } else {
        debug!("Generated batch number: {}", masked_value);
        Ok(masked_value as i32)
    }
}

async fn fetch_l2_blocks_and_transactions(
    start_block: u64,
    end_block: u64,
    batch_number: i32,
    timestamp: DateTime<Utc>,
) -> Result<
    (
        Vec<twine_batch_l2_blocks::ActiveModel>,
        Vec<twine_batch_l2_transactions::ActiveModel>,
    ),
    Report,
> {
    let rpc_url = env::var("TWINE__HTTP_RPC_URL")
        .unwrap_or_else(|_| "https://rpc1.twine.limited".to_string());
    info!(
        "Fetching L2 blocks and transactions from RPC: {} for batch_number: {}",
        rpc_url, batch_number
    );
    let provider = ProviderBuilder::new().on_http(rpc_url.parse().unwrap());

    let mut l2_blocks = Vec::new();
    let mut l2_transactions = Vec::new();

    for block_num in start_block..=end_block {
        info!(
            "Fetching block number: {} for batch_number: {}",
            block_num, batch_number
        );
        let block = provider
            .get_block_by_number(block_num.into(), true.into())
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch block {} for batch_number {}: {:?}",
                    block_num, batch_number, e
                );
                eyre::eyre!("Failed to fetch block {}: {:?}", block_num, e)
            })?;

        let block = block.ok_or_else(|| {
            error!(
                "Block {} not found for batch_number {}",
                block_num, batch_number
            );
            eyre::eyre!("Block {} not found", block_num)
        })?;
        let block_hash = block.header.hash;
        debug!("Fetched block {} with hash: {:?}", block_num, block_hash);

        let block_model = twine_batch_l2_blocks::ActiveModel {
            batch_number: Set(batch_number),
            hash: Set(block_hash.to_vec()),
            inserted_at: Set(timestamp.naive_utc()),
            updated_at: Set(timestamp.naive_utc()),
        };
        l2_blocks.push(block_model);
        debug!(
            "Created twine_batch_l2_blocks model for block {}",
            block_num
        );

        match &block.transactions {
            BlockTransactions::Full(transactions) => {
                info!(
                    "Processing {} full transactions in block {}",
                    transactions.len(),
                    block_num
                );
                for (index, tx) in transactions.iter().enumerate() {
                    let tx_hash = tx.inner.tx_hash().to_vec();
                    debug!(
                        "Processing transaction {}/{} in block {}: hash={:?}",
                        index + 1,
                        transactions.len(),
                        block_num,
                        tx_hash
                    );
                    let tx_model = twine_batch_l2_transactions::ActiveModel {
                        batch_number: Set(batch_number),
                        hash: Set(tx_hash),
                        inserted_at: Set(timestamp.naive_utc()),
                        updated_at: Set(timestamp.naive_utc()),
                    };
                    l2_transactions.push(tx_model);
                }
            }
            BlockTransactions::Hashes(hashes) => {
                info!(
                    "Processing {} transaction hashes in block {}",
                    hashes.len(),
                    block_num
                );
                for (index, tx_hash) in hashes.iter().enumerate() {
                    debug!(
                        "Processing transaction hash {}/{} in block {}: hash={:?}",
                        index + 1,
                        hashes.len(),
                        block_num,
                        tx_hash
                    );
                    let tx_model = twine_batch_l2_transactions::ActiveModel {
                        batch_number: Set(batch_number),
                        hash: Set(tx_hash.to_vec()),
                        inserted_at: Set(timestamp.naive_utc()),
                        updated_at: Set(timestamp.naive_utc()),
                    };
                    l2_transactions.push(tx_model);
                }
            }
            BlockTransactions::Uncle => {
                info!(
                    "Block {} is an uncle block, no transactions for batch_number {}",
                    block_num, batch_number
                );
            }
        }
    }
    info!(
        "Successfully collected {} blocks and {} transactions for batch_number {}",
        l2_blocks.len(),
        l2_transactions.len(),
        batch_number
    );
    Ok((l2_blocks, l2_transactions))
}

pub async fn parse_log(
    log: Log,
    db: &DatabaseConnection,
    blockscout_db: &DatabaseConnection,
    chain_id: u64,
) -> Result<ParsedLog, Report> {
    info!(
        "Parsing log: block_number={:?}, tx_hash={:?}, topic0={:?}",
        log.block_number,
        log.transaction_hash,
        log.topic0()
    );

    let tx_hash = log
        .transaction_hash
        .ok_or(ParserError::MissingTransactionHash)?;
    let tx_hash_str = format!("{tx_hash:?}");
    debug!("Extracted transaction hash: {}", tx_hash_str);

    let block_number = log.block_number.ok_or(ParserError::MissingBlockNumber)? as i64;
    debug!("Extracted block number: {}", block_number);

    let timestamp = log
        .block_timestamp
        .and_then(|ts| chrono::DateTime::<Utc>::from_timestamp(ts as i64, 0))
        .unwrap_or_else(|| {
            tracing::warn!("Missing or invalid block timestamp in log. Using default timestamp.");
            Utc::now()
        });
    debug!("Determined timestamp: {}", timestamp);

    match log.topic0() {
        Some(sig) => match *sig {
            L1MessageQueue::QueueDepositTransaction::SIGNATURE_HASH => {
                info!("Decoding QueueDepositTransaction event");
                let decoded = log
                    .log_decode::<L1MessageQueue::QueueDepositTransaction>()
                    .map_err(|e| ParserError::DecodeError {
                        event_type: "QueueDepositTransaction",
                        source: Box::new(e),
                    })?;
                let data = decoded.inner.data;
                debug!("Decoded QueueDepositTransaction: nonce={}, chain_id={}", data.nonce, data.chainId);
                let model = DbModel::L1Deposit(l1_deposit::ActiveModel {
                    nonce: Set(data.nonce.try_into().unwrap()),
                    chain_id: Set(data.chainId.try_into().unwrap()),
                    block_number: Set(Some(data.blockNumber.try_into().unwrap())),
                    slot_number: Set(None),
                    from: Set(format!("{:?}", data.from)),
                    to_twine_address: Set(format!("{:?}", data.toTwineAddress)),
                    l1_token: Set(format!("{:?}", data.l1Token)),
                    l2_token: Set(format!("{:?}", data.l2Token)),
                    tx_hash: Set(tx_hash_str.clone()),
                    amount: Set(data.amount.to_string()),
                    created_at: Set(timestamp.into()),
                });
                info!("Created L1Deposit model: nonce={}, chain_id={}", data.nonce, data.chainId);
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
                    tx_hash: Set(tx_hash_str.clone()),
                    nonce: Set(data.nonce.try_into().unwrap()),
                    chain_id: Set(data.chainId.try_into().unwrap()),
                    block_number: Set(Some(data.blockNumber.try_into().unwrap())),
                    slot_number: Set(None),
                    l1_token: Set(format!("{:?}", data.l1Token)),
                    l2_token: Set(format!("{:?}", data.l2Token)),
                    from: Set(format!("{:?}", data.from)),
                    to_twine_address: Set(format!("{:?}", data.toTwineAddress)),
                    amount: Set(data.amount.to_string()),
                    created_at: Set(timestamp.into()),
                });
                info!("Created L1Withdraw model: nonce={}, chain_id={}", data.nonce, data.chainId);
                Ok(ParsedLog {
                    model,
                    block_number,
                })
            }
            FinalizeWithdrawERC20::SIGNATURE_HASH => {
                info!("Decoding FinalizeWithdrawERC20 event");
                let decoded = log.log_decode::<FinalizeWithdrawERC20>().map_err(|e| {
                    ParserError::DecodeError {
                        event_type: "FinalizeWithdrawERC20",
                        source: Box::new(e),
                    }
                })?;
                let data = decoded.inner.data;
                debug!("Decoded FinalizeWithdrawERC20: nonce={}, chain_id={}", data.nonce, data.chainId);
                let model = DbModel::L2Withdraw(l2_withdraw::ActiveModel {
                    chain_id: Set(data.chainId.try_into().unwrap()),
                    nonce: Set(data.nonce.try_into().unwrap()),
                    block_number: Set(Some(data.blockNumber.try_into().unwrap())),
                    slot_number: Set(None),
                    tx_hash: Set(tx_hash_str.clone()),
                    created_at: Set(timestamp.into()),
                });
                info!("Created L2Withdraw model from FinalizeWithdrawERC20: nonce={}, chain_id={}", data.nonce, data.chainId);
                Ok(ParsedLog {
                    model,
                    block_number,
                })
            }
            FinalizeWithdrawETH::SIGNATURE_HASH => {
                info!("Decoding FinalizeWithdrawETH event");
                let decoded = log.log_decode::<FinalizeWithdrawETH>().map_err(|e| {
                    ParserError::DecodeError {
                        event_type: "FinalizeWithdrawETH",
                        source: Box::new(e),
                    }
                })?;
                let data = decoded.inner.data;
                debug!("Decoded FinalizeWithdrawETH: nonce={}, chain_id={}", data.nonce, data.chainId);
                let model = DbModel::L2Withdraw(l2_withdraw::ActiveModel {
                    chain_id: Set(data.chainId.try_into().unwrap()),
                    nonce: Set(data.nonce.try_into().unwrap()),
                    block_number: Set(Some(data.blockNumber.try_into().unwrap())),
                    slot_number: Set(None),
                    tx_hash: Set(tx_hash_str.clone()),
                    created_at: Set(timestamp.into()),
                });
                info!("Created L2Withdraw model from FinalizeWithdrawETH: nonce={}, chain_id={}", data.nonce, data.chainId);
                Ok(ParsedLog {
                    model,
                    block_number,
                })
            }
            CommitBatch::SIGNATURE_HASH => {
                info!("Decoding CommitBatch event");
                let decoded =
                    log.log_decode::<CommitBatch>()
                        .map_err(|e| ParserError::DecodeError {
                            event_type: "CommitBatch",
                            source: Box::new(e),
                        })?;
                let data = decoded.inner.data;
                let root_hash = format!("{:?}", data.batchHash);
                let chain_id_decimal = Decimal::from_i64(chain_id as i64).unwrap();
                let start_block =
                    data.startBlock
                        .try_into()
                        .map_err(|_| ParserError::NumberOverflow {
                            value: data.startBlock,
                        })?;
                let end_block =
                    data.endBlock
                        .try_into()
                        .map_err(|_| ParserError::NumberOverflow {
                            value: data.endBlock,
                        })?;
                debug!("Decoded CommitBatch: start_block={}, end_block={}, batch_hash={}", start_block, end_block, root_hash);

                // Generate the batch number first
                let batch_number = generate_batch_number(data.startBlock, data.endBlock)?;
                info!("Generated batch_number: {} for CommitBatch event", batch_number);

                // Check if a batch with this batch_number already exists
                debug!("Checking for existing batch with batch_number: {}", batch_number);
                let existing_batch = twine_transaction_batch::Entity::find()
                    .filter(twine_transaction_batch::Column::Number.eq(batch_number))
                    .one(blockscout_db)
                    .await?;
                let batch_exists = existing_batch.is_some();
                info!("Batch existence check: batch_number={} exists={}", batch_number, batch_exists);

                let (l2_blocks, l2_transactions) = if batch_exists {
                    info!(
                        "Batch number {} already exists in twine_transaction_batch, skipping L2 blocks and transactions generation",
                        batch_number
                    );
                    (Vec::new(), Vec::new())
                } else {
                    fetch_l2_blocks_and_transactions(
                        data.startBlock,
                        data.endBlock,
                        batch_number,
                        timestamp,
                    )
                    .await?
                };

                let batch_model = if batch_exists {
                    info!(
                        "Skipping batch insertion for batch_number: {} due to existing record",
                        batch_number
                    );
                    None
                } else {
                    let model = DbModel::TwineTransactionBatch {
                        model: twine_transaction_batch::ActiveModel {
                            number: Set(batch_number),
                            timestamp: Set(timestamp.naive_utc()),
                            start_block: Set(start_block),
                            end_block: Set(end_block),
                            root_hash: Set(
                                alloy::hex::decode(root_hash.trim_start_matches("0x")).unwrap()
                            ),
                            inserted_at: Set(timestamp.naive_utc()),
                            updated_at: Set(timestamp.naive_utc()),
                        },
                        chain_id: chain_id_decimal,
                        tx_hash: tx_hash_str.clone(),
                        l2_blocks,
                        l2_transactions,
                    };
                    info!("Created TwineTransactionBatch model: batch_number={}, start_block={}, end_block={}", batch_number, start_block, end_block);
                    Some(model)
                };

                if batch_model.is_none() {
                    let detail_model = DbModel::TwineTransactionBatchDetail(
                        twine_transaction_batch_detail::ActiveModel {
                            batch_number: Set(batch_number),
                            l2_transaction_count: Set(0),
                            l2_fair_gas_price: Set(Decimal::from_i32(0).unwrap()),
                            chain_id: Set(chain_id_decimal),
                            l1_transaction_count: Set(0),
                            l1_gas_price: Set(Decimal::from_i32(0).unwrap()),
                            commit_id: Set(None),
                            execute_id: Set(None),
                            inserted_at: Set(timestamp.naive_utc()),
                            updated_at: Set(timestamp.naive_utc()),
                            ..Default::default()
                        },
                    );
                    info!("Created TwineTransactionBatchDetail model as fallback for batch_number: {}", batch_number);
                    return Ok(ParsedLog {
                        model: detail_model,
                        block_number,
                    });
                }

                Ok(ParsedLog {
                    model: batch_model.unwrap(),
                    block_number,
                })
            }
            FinalizedBatch::SIGNATURE_HASH => {
                info!("Decoding FinalizedBatch event");
                let decoded =
                    log.log_decode::<FinalizedBatch>()
                        .map_err(|e| ParserError::DecodeError {
                            event_type: "FinalizedBatch",
                            source: Box::new(e),
                        })?;
                let data = decoded.inner.data;
                debug!("Decoded FinalizedBatch: start_block={}, end_block={}", data.startBlock, data.endBlock);

                let batch_number = generate_batch_number(data.startBlock, data.endBlock)?;
                info!("Generated batch_number: {} for FinalizedBatch event", batch_number);

                debug!("Fetching batch with batch_number: {}", batch_number);
                let batch = twine_transaction_batch::Entity::find()
                    .filter(twine_transaction_batch::Column::Number.eq(batch_number))
                    .one(blockscout_db)
                    .await?
                    .ok_or_else(|| {
                        error!("Batch not found for batch_number: {}", batch_number);
                        ParserError::FinalizedBeforeCommit {
                            start_block: data.startBlock,
                            end_block: data.endBlock,
                            batch_hash: format!("{:?}", data.batchHash),
                        }
                    })?;
                info!("Found batch for batch_number: {}", batch.number);

                let lifecycle_model = DbModel::TwineLifecycleL1Transactions {
                    model: twine_lifecycle_l1_transactions::ActiveModel {
                        hash: Set(alloy::hex::decode(tx_hash_str.trim_start_matches("0x")).unwrap()),
                        chain_id: Set(Decimal::from_i64(chain_id as i64).unwrap()),
                        timestamp: Set(timestamp.naive_utc()),
                        inserted_at: Set(timestamp.naive_utc()),
                        updated_at: Set(timestamp.naive_utc()),
                        ..Default::default()
                    },
                    batch_number: batch.number,
                };
                info!("Created TwineLifecycleL1Transactions model for batch_number: {}", batch.number);
                Ok(ParsedLog {
                    model: lifecycle_model,
                    block_number,
                })
            }
            other => {
                error!("Unknown event signature: {}", other);
                Err(ParserError::UnknownEvent { signature: other }.into())
            }
        },
        None => {
            error!("No event signature found in log");
            Err(ParserError::UnknownEvent {
                signature: alloy::primitives::B256::ZERO,
            }
            .into())
        }
    }.map(|parsed_log| {
        info!("Successfully parsed log into model: {:?}", std::mem::discriminant(&parsed_log.model));
        parsed_log
    })
}
