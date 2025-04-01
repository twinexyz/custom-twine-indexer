use alloy::hex;
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
use tracing::{debug, info};

use crate::entities::{
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
    #[borsh(skip)]
    pub signature: String,
    pub slot_number: u64,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FinalizedBatch {
    pub start_block: u64,
    pub end_block: u64,
    pub chain_id: u64,
    pub batch_hash: [u8; 32], // Added to match the event
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
        chain_id: Decimal, // Aligned with migration and EVM parser
        tx_hash: String,
        l2_blocks: Vec<twine_batch_l2_blocks::ActiveModel>,
        l2_transactions: Vec<twine_batch_l2_transactions::ActiveModel>,
    },
    TwineLifecycleL1Transactions {
        model: twine_lifecycle_l1_transactions::ActiveModel,
        batch_number: i32,
    },
    UpdateTwineTransactionBatchDetail {
        start_block: i32,          // Aligned with migration (integer)
        end_block: i32,            // Aligned with migration (integer)
        chain_id: Decimal,         // Aligned with migration (decimal)
        l1_transaction_count: i32, // Aligned with migration (integer)
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

fn generate_block_hash(block_number: u64) -> Vec<u8> {
    let input = format!("block:{}", block_number);
    let digest = blake3::hash(input.as_bytes());
    digest.as_bytes().to_vec()
}

fn generate_transaction_hash(block_number: u64, tx_index: u64) -> Vec<u8> {
    let input = format!("tx:{}:{}", block_number, tx_index);
    let digest = blake3::hash(input.as_bytes());
    digest.as_bytes().to_vec()
}

pub async fn parse_log(
    logs: &[String],
    signature: Option<String>,
    db: &DatabaseConnection,
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
        return None;
    };
    let Some(encoded_data) = encoded_data else {
        info!(
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

            let existing_batch = twine_transaction_batch::Entity::find()
                .filter(twine_transaction_batch::Column::StartBlock.eq(start_block))
                .filter(twine_transaction_batch::Column::EndBlock.eq(end_block))
                .one(db)
                .await
                .ok()?;
            let batch_number = if let Some(batch) = existing_batch {
                info!("Found existing batch: {:?}", batch);
                batch.number
            } else {
                let num = generate_number(commit.start_block, commit.end_block)
                    .expect("Failed to generate batch number");
                info!("Generated new batch number: {:?}", num);
                num
            };

            let batch_exists_in_l2_blocks = twine_batch_l2_blocks::Entity::find()
                .filter(twine_batch_l2_blocks::Column::BatchNumber.eq(batch_number))
                .one(db)
                .await
                .ok()?
                .is_some();

            let (l2_blocks, l2_transactions) = if batch_exists_in_l2_blocks {
                info!(
                    "Batch number {} already exists in twine_batch_l2_blocks, skipping L2 blocks and transactions generation",
                    batch_number
                );
                (Vec::new(), Vec::new())
            } else {
                let mut l2_blocks = Vec::new();
                let mut l2_transactions = Vec::new();
                for block_num in commit.start_block..=commit.end_block {
                    let block_hash = generate_block_hash(block_num);
                    let block_model = twine_batch_l2_blocks::ActiveModel {
                        batch_number: Set(batch_number),
                        hash: Set(block_hash),
                        created_at: Set(timestamp.into()),
                        updated_at: Set(timestamp.into()),
                    };
                    l2_blocks.push(block_model);

                    let tx_hash = generate_transaction_hash(block_num, 0);
                    let tx_model = twine_batch_l2_transactions::ActiveModel {
                        batch_number: Set(batch_number),
                        hash: Set(tx_hash),
                        created_at: Set(timestamp.into()),
                        updated_at: Set(timestamp.into()),
                    };
                    l2_transactions.push(tx_model);
                }
                (l2_blocks, l2_transactions)
            };

            let model = DbModel::TwineTransactionBatch {
                model: twine_transaction_batch::ActiveModel {
                    number: Set(batch_number),
                    timestamp: Set(timestamp.into()),
                    start_block: Set(start_block),
                    end_block: Set(end_block),
                    root_hash: Set(Vec::new()), // Placeholder; Solana event lacks root_hash
                    created_at: Set(timestamp.into()),
                    updated_at: Set(timestamp.into()),
                },
                chain_id,
                tx_hash,
                l2_blocks,
                l2_transactions,
            };
            Some(ParsedEvent {
                model,
                slot_number: commit.slot_number as i64,
            })
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
            let batch = twine_transaction_batch::Entity::find()
                .filter(twine_transaction_batch::Column::StartBlock.eq(start_block))
                .filter(twine_transaction_batch::Column::EndBlock.eq(end_block))
                .one(db)
                .await
                .ok()?
                .ok_or_else(|| {
                    eyre::eyre!(
                        "Batch not found for start_block: {}, end_block: {}",
                        finalize.start_block,
                        finalize.end_block
                    )
                })
                .ok()?;
            let model = DbModel::TwineLifecycleL1Transactions {
                model: twine_lifecycle_l1_transactions::ActiveModel {
                    id: NotSet,
                    hash: Set(tx_hash.as_bytes().to_vec()), // Convert to binary
                    chain_id: Set(Decimal::from_i64(finalize.chain_id as i64).unwrap()),
                    timestamp: Set(timestamp.into()),
                    created_at: Set(timestamp.into()),
                    updated_at: Set(timestamp.into()),
                },
                batch_number: batch.number,
            };
            Some(ParsedEvent {
                model,
                slot_number: finalize.slot_number as i64,
            })
        }
        "commit_and_finalize_transaction" => {
            let final_tx =
                parse_borsh::<FinalizedTransaction>(&encoded_data, signature.clone()).ok()?;
            let l1_transaction_count: i32 =
                match (final_tx.deposit_count + final_tx.withdraw_count).try_into() {
                    Ok(value) => value,
                    Err(_) => {
                        debug!(
                            "L1 transaction count overflow: {}",
                            final_tx.deposit_count + final_tx.withdraw_count
                        );
                        return None;
                    }
                };
            let start_block: i32 = match final_tx.start_block.try_into() {
                Ok(value) => value,
                Err(_) => {
                    debug!(
                        "Start block overflow for commit_and_finalize_transaction: {}",
                        final_tx.start_block
                    );
                    return None;
                }
            };
            let end_block: i32 = match final_tx.end_block.try_into() {
                Ok(value) => value,
                Err(_) => {
                    debug!(
                        "End block overflow for commit_and_finalize_transaction: {}",
                        final_tx.end_block
                    );
                    return None;
                }
            };
            let model = DbModel::UpdateTwineTransactionBatchDetail {
                start_block,
                end_block,
                chain_id: Decimal::from_i64(final_tx.chain_id as i64).unwrap(),
                l1_transaction_count,
            };
            Some(ParsedEvent {
                model,
                slot_number: final_tx.slot_number as i64,
            })
        }
        _ => None,
    }
}
