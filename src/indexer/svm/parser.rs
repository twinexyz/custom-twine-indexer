use alloy::hex;
use base64::{engine::general_purpose, Engine as _};
use borsh::{BorshDeserialize, BorshSerialize};
use chrono::Utc;
use eyre::Result;
use sea_orm::{
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::entities::{
    l1_deposit, l1_withdraw, l2_withdraw, twine_lifecycle_l1_transactions, twine_transaction_batch,
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
    pub batch_hash: String, // Must match EVM's batchHash
    #[borsh(skip)]
    pub signature: String,
    pub slot_number: u64,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FinalizedBatch {
    pub start_block: u64,
    pub end_block: u64,
    pub batch_hash: String,
    pub chain_id: u64,
    #[borsh(skip)]
    pub signature: String,
    pub slot_number: u64,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FinalizedTransaction {
    pub start_block: u64,
    pub end_block: u64,
    pub deposit_count: u64,
    pub withdraw_count: u64,
    pub chain_id: u64,
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
        chain_id: i64,
        tx_hash: String,
    },
    TwineLifecycleL1Transactions {
        model: twine_lifecycle_l1_transactions::ActiveModel,
        batch_number: i32,
    },
    UpdateTwineLifecycleL1Transactions {
        start_block: i64,
        end_block: i64,
        chain_id: i64,
        l1_transaction_count: i64,
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

pub fn generate_number(start_block: u64, end_block: u64, batch_hash: &str) -> Result<i32> {
    let input = format!("{}:{}:{}", start_block, end_block, batch_hash);
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
            let existing_batch = twine_transaction_batch::Entity::find()
                .filter(twine_transaction_batch::Column::StartBlock.eq(commit.start_block as i64))
                .filter(twine_transaction_batch::Column::EndBlock.eq(commit.end_block as i64))
                .filter(twine_transaction_batch::Column::RootHash.eq(commit.batch_hash.clone()))
                .one(db)
                .await
                .ok()?;
            let batch_number = if let Some(batch) = existing_batch {
                info!("Found existing batch: {:?}", batch);
                batch.number
            } else {
                let num = generate_number(commit.start_block, commit.end_block, &commit.batch_hash)
                    .expect("Failed to generate batch number");
                info!("Generated new batch number: {:?}", num);
                num
            };
            let model = DbModel::TwineTransactionBatch {
                model: twine_transaction_batch::ActiveModel {
                    number: Set(batch_number),
                    timestamp: Set(timestamp.into()),
                    start_block: Set(commit.start_block as i64),
                    end_block: Set(commit.end_block as i64),
                    root_hash: Set(commit.batch_hash.clone()),
                    created_at: Set(timestamp.into()),
                    updated_at: Set(timestamp.into()),
                },
                chain_id: commit.chain_id as i64,
                tx_hash,
            };
            Some(ParsedEvent {
                model,
                slot_number: commit.slot_number as i64,
            })
        }
        "finalize_batch" => {
            let finalize = parse_borsh::<FinalizedBatch>(&encoded_data, signature.clone()).ok()?;
            let batch = twine_transaction_batch::Entity::find()
                .filter(twine_transaction_batch::Column::StartBlock.eq(finalize.start_block as i64))
                .filter(twine_transaction_batch::Column::EndBlock.eq(finalize.end_block as i64))
                .filter(twine_transaction_batch::Column::RootHash.eq(finalize.batch_hash.clone()))
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
                    hash: Set(tx_hash),
                    chain_id: Set(finalize.chain_id as i64),
                    l1_transaction_count: Set(0),
                    l1_gas_price: Set(0.into()),
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
            let l1_transaction_count = (final_tx.deposit_count + final_tx.withdraw_count) as i64;
            let model = DbModel::UpdateTwineLifecycleL1Transactions {
                start_block: final_tx.start_block as i64,
                end_block: final_tx.end_block as i64,
                chain_id: final_tx.chain_id as i64,
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
