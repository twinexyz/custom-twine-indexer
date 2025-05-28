use std::{collections::HashMap, sync::Arc};

use base64::{engine::general_purpose, Engine};
use borsh::BorshDeserialize;
use chrono::{DateTime, Utc};
use common::config::{ChainConfig, SvmConfig};
use database::{
    blockscout_entities::{twine_transaction_batch, twine_transaction_batch_detail},
    client::DbClient,
    entities::{l1_deposit, l1_withdraw, l2_withdraw},
    DbOperations,
};
use evm::provider::EvmProvider;
use eyre::Error;
use num_traits::FromPrimitive;
use sea_orm::{
    prelude::Decimal,
    strum::{self},
    ActiveValue::Set,
    IntoActiveModel,
};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status_client_types::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransactionWithStatusMeta,
};
use tracing::{debug, error, info, instrument, warn};

use crate::parser::{
    CommitBatch, DepositSuccessful, FinalizeNativeWithdrawal, FinalizeSplWithdrawal,
    FinalizedBatch, ForcedWithdrawSuccessful, SolanaEvents,
};

pub struct SolanaEventHandler {
    db_client: Arc<DbClient>,
    config: SvmConfig,
    twine_provider: EvmProvider,
}

pub struct LogContext {
    pub tx_hash_str: String,
    // pub block_number: i64,
    pub timestamp: DateTime<Utc>,
    pub event_type: SolanaEvents,
    pub encoded_data: String,
}

pub struct LogProcessed {
    pub slot: u64,
}

impl SolanaEventHandler {
    pub fn new(db_client: Arc<DbClient>, config: SvmConfig, twine_provider: EvmProvider) -> Self {
        Self {
            db_client,
            config,
            twine_provider,
        }
    }

    pub fn chain_id(&self) -> u64 {
        self.config.common.chain_id
    }

    pub fn get_chain_config(&self) -> ChainConfig {
        self.config.common.clone()
    }

    pub fn get_program_addresses(&self) -> Vec<Pubkey> {
        let twine_chain_id =
            Pubkey::from_str_const(&self.config.twine_chain_program_address.clone());
        let gateway_id =
            Pubkey::from_str_const(&self.config.tokens_gateway_program_address.clone());
        return vec![twine_chain_id, gateway_id];
    }

    #[instrument(skip_all, fields(CHAIN = %self.chain_id()))]
    pub async fn handle_event(
        &self,
        logs: &[String],
        signature: Option<String>,
    ) -> eyre::Result<Vec<DbOperations>> {
        let Some(parsed) = self.extract_log(&logs, signature) else {
            info!("No parsable event found in logs");
            return Ok(vec![]);
        };

        info!(
            "Event received: {} for slot: {}",
            parsed.event_type.as_str(),
            parsed.tx_hash_str
        );

        let mut slot_number = 0;
        let mut operations = Vec::new();

        match parsed.event_type {
            SolanaEvents::SplDeposit | SolanaEvents::NativeDeposit => {
                let operation = self.handle_deposit(parsed).await?;
                operations.push(operation);
            }

            SolanaEvents::NativeWithdrawal | SolanaEvents::SplWithdrawal => {
                let operation = self.handle_withdrawal(parsed).await?;
                operations.push(operation);
            }
            SolanaEvents::FinalizeSplWithdrawal => {
                let operation = self.handle_finalize_spl_withdraw(parsed).await?;
                operations.push(operation);
            }

            SolanaEvents::FinalizeNativeWithdrawal => {
                let operation = self.handle_finalize_native_withdraw(parsed).await?;
                operations.push(operation);
            }

            SolanaEvents::CommitBatch => {
                let operation = self.handle_commit_batch(parsed).await?;
                operations.push(operation);
            }

            SolanaEvents::FinalizeBatch => {
                let operation = self.handle_finalize_batch(parsed).await?;
                operations.push(operation);
            }

            SolanaEvents::CommitAndFinalizeTransaction => {}

            _ => {
                info!("Unknown event to handle!")
            }
        }

        Ok(operations)
    }

    async fn handle_deposit(&self, parsed: LogContext) -> eyre::Result<DbOperations> {
        let deposit = self.parse_borsh::<DepositSuccessful>(&parsed.encoded_data)?;

        let model = l1_deposit::ActiveModel {
            nonce: Set(deposit.nonce as i64),
            chain_id: Set(deposit.chain_id as i64),
            block_number: Set(None),
            slot_number: Set(Some(deposit.slot_number as i64)),
            from: Set(deposit.from_l1_pubkey),
            to_twine_address: Set(deposit.to_twine_address),
            l1_token: Set(deposit.l1_token),
            l2_token: Set(deposit.l2_token),
            tx_hash: Set(parsed.tx_hash_str),
            amount: Set(deposit.amount),
            created_at: Set(parsed.timestamp.into()),
        };

        let operation = DbOperations::L1Deposits(model);

        // let _ = self.db_client.insert_l1_deposits(model).await?;

        Ok(operation)
    }

    async fn handle_withdrawal(&self, parsed: LogContext) -> eyre::Result<DbOperations> {
        let withdrawal = self.parse_borsh::<ForcedWithdrawSuccessful>(&parsed.encoded_data)?;
        let model = l1_withdraw::ActiveModel {
            nonce: Set(withdrawal.nonce as i64),
            chain_id: Set(withdrawal.chain_id as i64),
            block_number: Set(None),
            slot_number: Set(Some(withdrawal.slot_number as i64)),
            from: Set(withdrawal.from_twine_address),
            to_twine_address: Set(withdrawal.to_l1_pub_key),
            l1_token: Set(withdrawal.l1_token),
            l2_token: Set(withdrawal.l2_token),
            tx_hash: Set(parsed.tx_hash_str),
            amount: Set(withdrawal.amount),
            created_at: Set(parsed.timestamp.into()),
        };

        let operation = DbOperations::L1Withdraw(model);

        Ok(operation)
    }

    async fn handle_finalize_native_withdraw(
        &self,
        parsed_log: LogContext,
    ) -> eyre::Result<DbOperations> {
        let native = self.parse_borsh::<FinalizeNativeWithdrawal>(&parsed_log.encoded_data)?;

        let model = l2_withdraw::ActiveModel {
            nonce: Set(native.nonce as i64),
            chain_id: Set(native.chain_id as i64),
            block_number: Set(None),
            slot_number: Set(Some(native.slot_number as i64)),
            tx_hash: Set(native.signature),
            created_at: Set(parsed_log.timestamp.into()),
        };

        let operation = DbOperations::L2Withdraw(model);
        Ok(operation)
    }

    async fn handle_finalize_spl_withdraw(
        &self,
        parsed_log: LogContext,
    ) -> eyre::Result<DbOperations> {
        let spl = self.parse_borsh::<FinalizeSplWithdrawal>(&parsed_log.encoded_data)?;

        let model = l2_withdraw::ActiveModel {
            nonce: Set(spl.nonce as i64),
            chain_id: Set(spl.chain_id as i64),
            block_number: Set(None),
            slot_number: Set(Some(spl.slot_number as i64)),
            tx_hash: Set(spl.signature),
            created_at: Set(parsed_log.timestamp.into()),
        };

        let operation = DbOperations::L2Withdraw(model);
        Ok(operation)
    }

    async fn handle_commit_batch(&self, parsed_log: LogContext) -> eyre::Result<DbOperations> {
        let commit = self.parse_borsh::<CommitBatch>(&parsed_log.encoded_data)?;

        let start_block = commit.start_block;
        let end_block = commit.end_block;
        let root_hash = vec![0u8; 32];
        let batch_length = end_block - start_block + 1; // as both end block and start block is inclusive

        //Build batch model
        let batch_model = twine_transaction_batch::ActiveModel {
            number: Set(start_block as i64),
            timestamp: Set(parsed_log.timestamp.naive_utc()),
            start_block: Set(start_block as i64),
            end_block: Set(end_block as i64),
            root_hash: Set(root_hash),
            ..Default::default()
        };

        let detail_model = twine_transaction_batch_detail::ActiveModel {
            batch_number: Set(start_block as i64),
            l1_transaction_count: Set(0),
            l2_transaction_count: Set(0),
            l1_gas_price: Set(Decimal::from_f64(0.0).unwrap()),
            l2_fair_gas_price: Set(Decimal::from_f64(0.0).unwrap()),
            chain_id: Set(Decimal::from_i64(self.chain_id() as i64).unwrap()),
            commit_transaction_hash: Set(Some(parsed_log.tx_hash_str.clone().into_bytes())),
            finalize_transaction_hash: Set(None),
            ..Default::default()
        };

        let mut l2_blocks = Vec::new();
        let mut l2_txs = Vec::new();

        //1. Check if batch already exists
        match self.db_client.get_batch_by_id(start_block as i64).await? {
            Some(existing_batch) => {
                //If the batch already exists, it means it already has its corresponding transactions and blocks as welll.
                // So we just need to put twine commit information in the table

                info!("Batch already exists so don't need to fetch blocks and transactions");
            }
            None => {
                // info!("First time encoutering this batch, so need to fecth blocks and transactions from twine");

                let blocks = self.db_client.get_blocks(start_block, end_block).await?;

                if blocks.len() != batch_length as usize {
                    error!(
                        "Fetched blocks length {:?} mismatched with batch length {:?}",
                        blocks.len(),
                        batch_length
                    );

                    return Err(eyre::eyre!(
                        "Fetched blocks length {:?} mismatched with batch length {:?}",
                        blocks.len(),
                        batch_length
                    ));
                } else {
                    let transactions = self
                        .db_client
                        .get_transactions(start_block, end_block)
                        .await?;

                    l2_blocks = blocks
                        .into_iter()
                        .map(|model| {
                            let mut am = model.into_active_model();
                            am.batch_number = Set(Some(start_block as i64));
                            am
                        })
                        .collect();

                    l2_txs = transactions
                        .into_iter()
                        .map(|model| {
                            let mut am = model.into_active_model();
                            am.batch_number = Set(Some(start_block as i64));
                            am
                        })
                        .collect();
                }
            }
        }

        let operation = DbOperations::CommitBatch {
            batch: batch_model,
            details: detail_model,
            blocks: l2_blocks,
            transactions: l2_txs,
        };

        Ok(operation)
    }

    async fn handle_finalize_batch(&self, parsed_log: LogContext) -> eyre::Result<DbOperations> {
        let finalize = self.parse_borsh::<FinalizedBatch>(&parsed_log.encoded_data)?;

        let start_block = finalize.start_block;
        let end_block = finalize.end_block;
        let root_hash = vec![0u8; 32];

        Ok(DbOperations::FinalizeBatch {
            finalize_hash: parsed_log.tx_hash_str.into_bytes(),
            batch_number: start_block as i64,
        })
    }

    fn parse_borsh<T: BorshDeserialize>(&self, encoded_data: &str) -> eyre::Result<T> {
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

        Ok(event)
    }

    fn extract_log(&self, logs: &[String], signature: Option<String>) -> Option<LogContext> {
        debug!("Parsing logs: {:?}", logs);

        let mut event_type = None;
        let mut encoded_data = None;

        for log in logs {
            match log.as_str() {
                "Program log: Instruction: NativeTokenDeposit" => {
                    event_type = Some(SolanaEvents::NativeDeposit)
                }
                "Program log: Instruction: SplTokensDeposit" => {
                    event_type = Some(SolanaEvents::SplDeposit)
                }
                "Program log: Instruction: ForcedNativeTokenWithdrawal" => {
                    event_type = Some(SolanaEvents::NativeWithdrawal)
                }
                "Program log: Instruction: ForcedSplTokenWithdrawal" => {
                    event_type = Some(SolanaEvents::SplWithdrawal)
                }
                "Program log: Instruction: FinalizeNativeWithdrawal" => {
                    event_type = Some(SolanaEvents::FinalizeNativeWithdrawal)
                }
                "Program log: Instruction: FinalizeSplWithdrawal" => {
                    event_type = Some(SolanaEvents::FinalizeSplWithdrawal)
                }
                "Program log: Instruction: CommitBatch" => {
                    event_type = Some(SolanaEvents::CommitBatch)
                }
                "Program log: Instruction: FinalizeBatch" => {
                    event_type = Some(SolanaEvents::FinalizeBatch)
                }
                "Program log: Instruction: CommitAndFinalizeTransaction" => {
                    event_type = Some(SolanaEvents::CommitAndFinalizeTransaction)
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
                event_type.as_str(),
                logs
            );
            return None;
        };

        debug!(
            "Identified event_type: {}, encoded_data: {}",
            event_type.as_str(),
            encoded_data
        );

        let timestamp = Utc::now();
        let tx_hash = signature.clone().unwrap_or_default();

        Some(LogContext {
            tx_hash_str: tx_hash,
            timestamp: timestamp,
            event_type: event_type,
            encoded_data,
        })
    }
}
