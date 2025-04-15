use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::BlockTransactions;
use anchor_client::solana_sdk::bs58;
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

use common::entities::{
    l1_deposit, l1_withdraw, l2_withdraw, twine_batch_l2_blocks, twine_batch_l2_transactions,
    twine_lifecycle_l1_transactions, twine_transaction_batch, twine_transaction_batch_detail,
};

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
    TwineLifecycleL1Transactions {
        model: twine_lifecycle_l1_transactions::ActiveModel,
        batch_number: i32,
    },
    UpdateTwineTransactionBatchDetail {
        start_block: i32,
        end_block: i32,
        chain_id: Decimal,
        l1_transaction_count: i32,
    },
}

#[derive(Debug)]
pub struct ParsedEvent {
    pub model: DbModel,
    pub slot_number: i64,
}

pub trait HasSignature {
    fn set_signature(&mut self, signature: String);
}

impl HasSignature for DepositSuccessful {
    fn set_signature(&mut self, signature: String) {
        self.signature = signature;
    }
}

impl HasSignature for ForcedWithdrawSuccessful {
    fn set_signature(&mut self, signature: String) {
        self.signature = signature;
    }
}

impl HasSignature for FinalizeNativeWithdrawal {
    fn set_signature(&mut self, signature: String) {
        self.signature = signature;
    }
}

impl HasSignature for FinalizeSplWithdrawal {
    fn set_signature(&mut self, signature: String) {
        self.signature = signature;
    }
}

impl HasSignature for CommitBatch {
    fn set_signature(&mut self, signature: String) {
        self.signature = signature;
    }
}

impl HasSignature for FinalizedBatch {
    fn set_signature(&mut self, signature: String) {
        self.signature = signature;
    }
}

impl HasSignature for FinalizedTransaction {
    fn set_signature(&mut self, signature: String) {
        self.signature = signature;
    }
}

pub fn parse_borsh<T: BorshDeserialize + HasSignature>(
    encoded_data: &str,
    signature: Option<String>,
) -> Result<T> {
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

    if let Some(sig) = signature {
        event.set_signature(sig);
    } else {
        event.set_signature(String::new());
    }

    Ok(event)
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

pub async fn parse_log(
    logs: &[String],
    signature: Option<String>,
    db: &DatabaseConnection,            // Local DB for deposits/withdrawals
    blockscout_db: &DatabaseConnection, // Blockscout DB for batch-related data
) -> Option<ParsedEvent> {
    debug!("Parsing logs: {:?}", logs);

    let mut event_type = None;
    let mut encoded_data = None;

    for log in logs {
        match log.as_str() {
            "Program log: Instruction: NativeTokenDeposit" => event_type = Some("native_deposit"),
            "Program log: Instruction: SplTokensDeposit" => event_type = Some("spl_deposit"),
            "Program log: Instruction: ForcedNativeTokenWithdrawal" => {
                event_type = Some("native_withdrawal")
            }
            "Program log: Instruction: ForcedSplTokenWithdrawal" => {
                event_type = Some("spl_withdrawal")
            }
            "Program log: Instruction: FinalizeNativeWithdrawal" => {
                event_type = Some("finalize_native_withdrawal")
            }
            "Program log: Instruction: FinalizeSplWithdrawal" => {
                event_type = Some("finalize_spl_withdrawal")
            }
            "Program log: Instruction: CommitBatch" => event_type = Some("commit_batch"),
            "Program log: Instruction: FinalizeBatch" => event_type = Some("finalize_batch"),
            "Program log: Instruction: CommitAndFinalizeTransaction" => {
                event_type = Some("commit_and_finalize_transaction")
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
            event_type, logs
        );
        return None;
    };

    debug!(
        "Identified event_type: {}, encoded_data: {}",
        event_type, encoded_data
    );

    let timestamp = Utc::now();
    let tx_hash = signature.clone().unwrap_or_default();

    match event_type {
        "native_deposit" | "spl_deposit" => {
            let deposit =
                parse_borsh::<DepositSuccessful>(&encoded_data, signature.clone()).ok()?;
            let model = DbModel::L1Deposit(l1_deposit::ActiveModel {
                nonce: Set(deposit.nonce as i64),
                chain_id: Set(deposit.chain_id as i64),
                block_number: Set(None),
                slot_number: Set(Some(deposit.slot_number as i64)),
                from: Set(deposit.from_l1_pubkey),
                to_twine_address: Set(deposit.to_twine_address),
                l1_token: Set(deposit.l1_token),
                l2_token: Set(deposit.l2_token),
                tx_hash: Set(deposit.signature),
                amount: Set(deposit.amount),
                created_at: Set(timestamp.into()),
            });
            Some(ParsedEvent {
                model,
                slot_number: deposit.slot_number as i64,
            })
        }
        "native_withdrawal" | "spl_withdrawal" => {
            let withdrawal =
                parse_borsh::<ForcedWithdrawSuccessful>(&encoded_data, signature.clone()).ok()?;
            let model = DbModel::L1Withdraw(l1_withdraw::ActiveModel {
                nonce: Set(withdrawal.nonce as i64),
                chain_id: Set(withdrawal.chain_id as i64),
                block_number: Set(None),
                slot_number: Set(Some(withdrawal.slot_number as i64)),
                from: Set(withdrawal.from_twine_address),
                to_twine_address: Set(withdrawal.to_l1_pub_key),
                l1_token: Set(withdrawal.l1_token),
                l2_token: Set(withdrawal.l2_token),
                tx_hash: Set(withdrawal.signature),
                amount: Set(withdrawal.amount),
                created_at: Set(timestamp.into()),
            });
            Some(ParsedEvent {
                model,
                slot_number: withdrawal.slot_number as i64,
            })
        }
        "finalize_native_withdrawal" => {
            let native =
                parse_borsh::<FinalizeNativeWithdrawal>(&encoded_data, signature.clone()).ok()?;
            let model = DbModel::L2Withdraw(l2_withdraw::ActiveModel {
                nonce: Set(native.nonce as i64),
                chain_id: Set(native.chain_id as i64),
                block_number: Set(None),
                slot_number: Set(Some(native.slot_number as i64)),
                tx_hash: Set(native.signature),
                created_at: Set(timestamp.into()),
            });
            Some(ParsedEvent {
                model,
                slot_number: native.slot_number as i64,
            })
        }
        "finalize_spl_withdrawal" => {
            let spl =
                parse_borsh::<FinalizeSplWithdrawal>(&encoded_data, signature.clone()).ok()?;
            let model = DbModel::L2Withdraw(l2_withdraw::ActiveModel {
                nonce: Set(spl.nonce as i64),
                chain_id: Set(spl.chain_id as i64),
                block_number: Set(None),
                slot_number: Set(Some(spl.slot_number as i64)),
                tx_hash: Set(spl.signature),
                created_at: Set(timestamp.into()),
            });
            Some(ParsedEvent {
                model,
                slot_number: spl.slot_number as i64,
            })
        }
        "commit_batch" => {
            let commit = parse_borsh::<CommitBatch>(&encoded_data, signature.clone()).ok()?;
            let start_block: i32 = match commit.start_block.try_into() {
                Ok(value) => value,
                Err(_) => {
                    debug!(
                        "Start block overflow for commit_batch: {}",
                        commit.start_block
                    );
                    return None;
                }
            };
            let end_block: i32 = match commit.end_block.try_into() {
                Ok(value) => value,
                Err(_) => {
                    debug!("End block overflow for commit_batch: {}", commit.end_block);
                    return None;
                }
            };
            let chain_id = Decimal::from_i64(commit.chain_id as i64).unwrap();

            // Generate batch number
            let batch_number = generate_number(commit.start_block, commit.end_block)
                .expect("Failed to generate batch number");

            // Check if batch already exists
            let existing_batch = twine_transaction_batch::Entity::find()
                .filter(twine_transaction_batch::Column::Number.eq(batch_number))
                .one(blockscout_db)
                .await
                .ok()?;

            let batch_exists = existing_batch.is_some();
            let batch_number = existing_batch.map(|b| b.number).unwrap_or(batch_number);

            let (l2_blocks, l2_transactions) = if batch_exists {
                info!(
                    "Batch number {} already exists in twine_transaction_batch, skipping L2 blocks and transactions generation",
                    batch_number
                );
                (Vec::new(), Vec::new())
            } else {
                let rpc_url = env::var("TWINE__HTTP_RPC_URL")
                    .unwrap_or_else(|_| "https://rpc1.twine.limited".to_string());
                info!("Fetching blocks from EVM RPC: {}", rpc_url);
                let provider = ProviderBuilder::new().on_http(rpc_url.parse().unwrap());

                let mut l2_blocks = Vec::new();
                let mut l2_transactions = Vec::new();

                for block_num in commit.start_block..=commit.end_block {
                    info!("Fetching block number: {}", block_num);
                    let block_result = provider
                        .get_block_by_number(block_num.into(), true.into())
                        .await;

                    let block = match block_result {
                        Ok(Some(block)) => block,
                        Ok(None) => {
                            error!("Block {} not found", block_num);
                            return None;
                        }
                        Err(e) => {
                            error!("Failed to fetch block {}: {:?}", block_num, e);
                            return None;
                        }
                    };

                    let block_hash = block.header.hash;

                    let block_model = twine_batch_l2_blocks::ActiveModel {
                        batch_number: Set(batch_number),
                        hash: Set(block_hash.to_vec()),
                        inserted_at: Set(timestamp.naive_utc()),
                        updated_at: Set(timestamp.naive_utc()),
                    };
                    l2_blocks.push(block_model);

                    match block.transactions {
                        BlockTransactions::Full(transactions) => {
                            for tx in transactions {
                                let tx_hash = tx.inner.tx_hash().to_vec();
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
                            for tx_hash in hashes {
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
                            info!("Block {} is an uncle block, no transactions", block_num);
                        }
                    }
                }
                info!(
                    "Collected {} blocks and {} transactions for batch {}",
                    l2_blocks.len(),
                    l2_transactions.len(),
                    batch_number
                );
                (l2_blocks, l2_transactions)
            };

            let model = DbModel::TwineTransactionBatch {
                model: twine_transaction_batch::ActiveModel {
                    number: Set(batch_number),
                    timestamp: Set(timestamp.naive_utc()),
                    start_block: Set(start_block),
                    end_block: Set(end_block),
                    root_hash: Set(Vec::new()),
                    inserted_at: Set(timestamp.naive_utc()),
                    updated_at: Set(timestamp.naive_utc()),
                },
                chain_id,
                tx_hash,
                l2_blocks,
                l2_transactions,
            };

            let parsed_event = ParsedEvent {
                model,
                slot_number: commit.slot_number as i64,
            };

            info!(
                "Parsed CommitBatch event: batch_number={}, start_block={}, end_block={}, slot_number={}",
                batch_number, start_block, end_block, commit.slot_number
            );

            Some(parsed_event)
        }
        "finalize_batch" => {
            let finalize = parse_borsh::<FinalizedBatch>(&encoded_data, signature.clone()).ok()?;
            let start_block: i32 = match finalize.start_block.try_into() {
                Ok(value) => value,
                Err(_) => {
                    debug!(
                        "Start block overflow for finalize_batch: {}",
                        finalize.start_block
                    );
                    return None;
                }
            };
            let end_block: i32 = match finalize.end_block.try_into() {
                Ok(value) => value,
                Err(_) => {
                    debug!(
                        "End block overflow for finalize_batch: {}",
                        finalize.end_block
                    );
                    return None;
                }
            };

            // Generate batch number
            let batch_number = generate_number(finalize.start_block, finalize.end_block)
                .expect("Failed to generate batch number");

            // Check if batch exists
            let batch = twine_transaction_batch::Entity::find()
                .filter(twine_transaction_batch::Column::Number.eq(batch_number))
                .one(blockscout_db)
                .await
                .ok()?
                .ok_or_else(|| {
                    error!(
                        "Finalized event indexed before commit for start_block: {}, end_block: {}, batch_hash: {:?}",
                        finalize.start_block, finalize.end_block, finalize.batch_hash
                    );
                    eyre::eyre!(
                        "Finalized event indexed before commit for start_block: {}, end_block: {}",
                        finalize.start_block, finalize.end_block
                    )
                })
                .ok()?;

            let decoded_hash = match bs58::decode(&tx_hash).into_vec() {
                Ok(hash) => {
                    if hash.len() != 64 {
                        error!(
                            "Invalid hash length for finalize_batch: expected 64, got {}",
                            hash.len()
                        );
                        return None;
                    }
                    hash
                }
                Err(e) => {
                    error!("Failed to decode Base58 tx_hash for finalize_batch: {}", e);
                    return None;
                }
            };

            let model = DbModel::TwineLifecycleL1Transactions {
                model: twine_lifecycle_l1_transactions::ActiveModel {
                    hash: Set(decoded_hash),
                    chain_id: Set(Decimal::from_i64(finalize.chain_id as i64).unwrap()),
                    timestamp: Set(timestamp.naive_utc()),
                    inserted_at: Set(timestamp.naive_utc()),
                    updated_at: Set(timestamp.naive_utc()),
                    ..Default::default()
                },
                batch_number: batch.number,
            };
            Some(ParsedEvent {
                model,
                slot_number: finalize.slot_number as i64,
            })
        }

        _ => None,
    }
}
