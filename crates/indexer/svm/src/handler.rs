use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
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
use generic_indexer::handler::ChainEventHandler;
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
    FinalizedBatch, ForcedWithdrawSuccessful, FoundEvent, SolanaEvents,
};

pub struct SolanaEventHandler {
    db_client: Arc<DbClient>,
    config: SvmConfig,
    twine_provider: EvmProvider,
}

#[async_trait]
impl ChainEventHandler for SolanaEventHandler {
    type LogType = FoundEvent;

    fn get_chain_config(&self) -> ChainConfig {
        self.config.common.clone()
    }

    fn get_db_client(&self) -> Arc<DbClient> {
        self.db_client.clone()
    }

    #[instrument(skip_all, fields(CHAIN = %self.chain_id()))]
    async fn handle_event(&self, log: FoundEvent) -> eyre::Result<Vec<DbOperations>> {
        let mut slot_number = 0;
        let mut operations = Vec::new();

        match log.event_type {
            SolanaEvents::SplDeposit | SolanaEvents::NativeDeposit => {
                let operation = self.handle_deposit(log).await?;
                operations.push(operation);
            }

            SolanaEvents::NativeWithdrawal | SolanaEvents::SplWithdrawal => {
                let operation = self.handle_withdrawal(log).await?;
                operations.push(operation);
            }
            SolanaEvents::FinalizeSplWithdrawal => {
                let operation = self.handle_finalize_spl_withdraw(log).await?;
                operations.push(operation);
            }

            SolanaEvents::FinalizeNativeWithdrawal => {
                let operation = self.handle_finalize_native_withdraw(log).await?;
                operations.push(operation);
            }

            SolanaEvents::CommitBatch => {
                let operation = self.handle_commit_batch(log).await?;
                operations.push(operation);
            }

            SolanaEvents::FinalizeBatch => {
                let operation = self.handle_finalize_batch(log).await?;
                operations.push(operation);
            }

            SolanaEvents::CommitAndFinalizeTransaction => {}

            _ => {
                info!("Unknown event to handle!")
            }
        }

        Ok(operations)
    }
}

impl SolanaEventHandler {
    pub fn new(db_client: Arc<DbClient>, config: SvmConfig, twine_provider: EvmProvider) -> Self {
        Self {
            db_client,
            config,
            twine_provider,
        }
    }

    pub fn get_program_addresses(&self) -> Vec<Pubkey> {
        let twine_chain_id =
            Pubkey::from_str_const(&self.config.twine_chain_program_address.clone());
        let gateway_id =
            Pubkey::from_str_const(&self.config.tokens_gateway_program_address.clone());
        return vec![twine_chain_id, gateway_id];
    }

    async fn handle_deposit(&self, parsed: FoundEvent) -> eyre::Result<DbOperations> {
        let deposit = parsed.parse_borsh::<DepositSuccessful>()?;

        let model = l1_deposit::ActiveModel {
            nonce: Set(deposit.nonce as i64),
            chain_id: Set(deposit.chain_id as i64),
            block_number: Set(None),
            slot_number: Set(Some(deposit.slot_number as i64)),
            from: Set(deposit.from_l1_pubkey),
            to_twine_address: Set(deposit.to_twine_address),
            l1_token: Set(deposit.l1_token),
            l2_token: Set(deposit.l2_token),
            tx_hash: Set(parsed.transaction_signature),
            amount: Set(deposit.amount),
            created_at: Set(parsed.timestamp.into()),
        };

        let operation = DbOperations::L1Deposits(model);

        // let _ = self.db_client.insert_l1_deposits(model).await?;

        Ok(operation)
    }

    async fn handle_withdrawal(&self, parsed: FoundEvent) -> eyre::Result<DbOperations> {
        let withdrawal = parsed.parse_borsh::<ForcedWithdrawSuccessful>()?;
        let model = l1_withdraw::ActiveModel {
            nonce: Set(withdrawal.nonce as i64),
            chain_id: Set(withdrawal.chain_id as i64),
            block_number: Set(None),
            slot_number: Set(Some(withdrawal.slot_number as i64)),
            from: Set(withdrawal.from_twine_address),
            to_twine_address: Set(withdrawal.to_l1_pub_key),
            l1_token: Set(withdrawal.l1_token),
            l2_token: Set(withdrawal.l2_token),
            tx_hash: Set(parsed.transaction_signature),
            amount: Set(withdrawal.amount),
            created_at: Set(parsed.timestamp.into()),
        };

        let operation = DbOperations::L1Withdraw(model);

        Ok(operation)
    }

    async fn handle_finalize_native_withdraw(
        &self,
        parsed: FoundEvent,
    ) -> eyre::Result<DbOperations> {
        let native = parsed.parse_borsh::<FinalizeNativeWithdrawal>()?;

        let model = l2_withdraw::ActiveModel {
            nonce: Set(native.nonce as i64),
            chain_id: Set(native.chain_id as i64),
            block_number: Set(None),
            slot_number: Set(Some(native.slot_number as i64)),
            tx_hash: Set(native.signature),
            created_at: Set(parsed.timestamp.into()),
        };

        let operation = DbOperations::L2Withdraw(model);
        Ok(operation)
    }

    async fn handle_finalize_spl_withdraw(&self, parsed: FoundEvent) -> eyre::Result<DbOperations> {
        let spl = parsed.parse_borsh::<FinalizeSplWithdrawal>()?;

        let model = l2_withdraw::ActiveModel {
            nonce: Set(spl.nonce as i64),
            chain_id: Set(spl.chain_id as i64),
            block_number: Set(None),
            slot_number: Set(Some(spl.slot_number as i64)),
            tx_hash: Set(spl.signature),
            created_at: Set(parsed.timestamp.into()),
        };

        let operation = DbOperations::L2Withdraw(model);
        Ok(operation)
    }

    async fn handle_commit_batch(&self, parsed: FoundEvent) -> eyre::Result<DbOperations> {
        let commit = parsed.parse_borsh::<CommitBatch>()?;

        let start_block = commit.start_block as u64;
        let end_block = commit.end_block as u64;
        let root_hash = vec![0u8; 32];
        let batch_length = end_block - start_block + 1; // as both end block and start block is inclusive

        //Build batch model
        let batch_model = twine_transaction_batch::ActiveModel {
            number: Set(start_block as i64),
            timestamp: Set(parsed.timestamp.naive_utc()),
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
            commit_transaction_hash: Set(Some(parsed.transaction_signature.clone().into_bytes())),
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

    async fn handle_finalize_batch(&self, parsed: FoundEvent) -> eyre::Result<DbOperations> {
        let finalize = parsed.parse_borsh::<FinalizedBatch>()?;

        let start_block = finalize.start_block;
        let end_block = finalize.end_block;
        let root_hash = vec![0u8; 32];

        Ok(DbOperations::FinalizeBatch {
            finalize_hash: parsed.transaction_signature.into_bytes(),
            batch_number: start_block as i64,
        })
    }
}

impl Clone for SolanaEventHandler {
    fn clone(&self) -> Self {
        Self {
            // chain_id: self.chain_id,
            config: self.config.clone(),
            db_client: self.db_client.clone(),
            twine_provider: self.twine_provider.clone(),
        }
    }
}
