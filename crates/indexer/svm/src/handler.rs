use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use base64::{engine::general_purpose, Engine};
use borsh::BorshDeserialize;
use chrono::{DateTime, Utc};
use common::config::{ChainConfig, SvmConfig};
use database::{
    blockscout_entities::{twine_transaction_batch, twine_transaction_batch_detail},
    client::DbClient,
    entities::{bridge_destination_transactions, bridge_source_transactions},
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
    CommitBatchEvent, FinalizeBatchEvent, ForcedWithdrawalSuccessfulEvent, L2WithdrawExecutedEvent,
    MessageTransactionEvent, SolanaEvent, SolanaLog,
};

pub struct SolanaEventHandler {
    db_client: Arc<DbClient>,
    config: SvmConfig,
    twine_provider: Arc<EvmProvider>,
}

#[async_trait]
impl ChainEventHandler for SolanaEventHandler {
    type LogType = SolanaLog;

    fn get_chain_config(&self) -> ChainConfig {
        self.config.common.clone()
    }

    #[instrument(skip_all, fields(CHAIN = %self.chain_id()))]
    async fn handle_event(&self, log: SolanaLog) -> eyre::Result<Vec<DbOperations>> {
        let mut slot_number = 0;
        let mut operations = Vec::new();

        info!(
            "Received event '{}' in block {}",
            log.event.get_event_type(),
            log.slot_number,
        );

        match log.event {
            SolanaEvent::MessageTransaction(event) => {
                let operation = self
                    .handle_deposit(event, log.signature, log.timestamp, log.slot_number)
                    .await?;
                operations.push(operation);
            }

            // SolanaEvent::RefundSuccessful(event) => {
            //     let operation = self.handle_refund(event, log.signature, log.timestamp, log.slot_number).await?;
            //     operations.push(operation);
            // }
            // SolanaEvent::ForcedWithdrawalSuccessful(event) => {
            //     let operation = self
            //         .handle_finalize_withdrawal(
            //             event,
            //             log.signature,
            //             log.timestamp,
            //             log.slot_number,
            //         )
            //         .await?;
            //     operations.push(operation);
            // }
            SolanaEvent::L2WithdrawExecuted(event) => {
                let operation: DbOperations = self
                    .handle_finalize_l2_withdrawal(
                        event,
                        log.signature,
                        log.timestamp,
                        log.slot_number,
                    )
                    .await?;
                operations.push(operation);
            }

            // SolanaEvents::NativeWithdrawal | SolanaEvents::SplWithdrawal => {
            //     let operation = self.handle_withdrawal(log).await?;
            //     operations.push(operation);
            // }
            // SolanaEvents::FinalizeSplWithdrawal => {
            //     let operation = self.handle_finalize_spl_withdraw(log).await?;
            //     operations.push(operation);
            // }

            // SolanaEvents::FinalizeNativeWithdrawal => {
            //     let operation = self.handle_finalize_native_withdraw(log).await?;
            //     operations.push(operation);
            // }
            SolanaEvent::CommitBatch(event) => {
                let operation = self
                    .handle_commit_batch(event, log.signature, log.timestamp, log.slot_number)
                    .await?;
                operations.push(operation);
            }

            SolanaEvent::FinalizeBatch(event) => {
                let operation = self
                    .handle_finalize_batch(event, log.signature, log.timestamp, log.slot_number)
                    .await?;
                operations.push(operation);
            }

            _ => {
                info!("Unknown event to handle! {:?}", log.event)
            }
        }

        Ok(operations)
    }
}

impl SolanaEventHandler {
    pub fn new(
        db_client: Arc<DbClient>,
        config: SvmConfig,
        twine_provider: Arc<EvmProvider>,
    ) -> Self {
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

    async fn handle_deposit(
        &self,
        event: MessageTransactionEvent,
        signature: String,
        timestamp: DateTime<Utc>,
        slot_number: u64,
    ) -> eyre::Result<DbOperations> {
        let model = bridge_source_transactions::ActiveModel {
            source_nonce: Set(event.nonce as i64),
            source_chain_id: Set(event.chain_id as i64),
            source_height: Set(Some(slot_number as i64)),
            source_from_address: Set(event.l1_pubkey),
            target_recipient_address: Set(Some(event.twine_address)),
            destination_token_address: Set(Some(event.l2_token)),
            source_token_address: Set(Some(event.l1_token)),
            source_tx_hash: Set(signature),
            source_event_timestamp: Set(timestamp.naive_utc()),
            event_type: Set(database::entities::sea_orm_active_enums::EventTypeEnum::Deposit),
            amount: Set(Some(event.amount)),
            ..Default::default()
        };

        let operation = DbOperations::BridgeSourceTransaction(model);

        // let _ = self.db_client.insert_l1_deposits(model).await?;

        Ok(operation)
    }

    async fn handle_withdrawal(
        &self,
        event: MessageTransactionEvent,
        signature: String,
        timestamp: DateTime<Utc>,
        slot_number: u64,
    ) -> eyre::Result<DbOperations> {
        let model = bridge_source_transactions::ActiveModel {
            source_nonce: Set(event.nonce as i64),
            source_chain_id: Set(event.chain_id as i64),
            source_height: Set(Some(slot_number as i64)),
            source_from_address: Set(event.l1_pubkey),
            target_recipient_address: Set(Some(event.twine_address)),
            destination_token_address: Set(Some(event.l2_token)),
            source_token_address: Set(Some(event.l1_token)),
            source_tx_hash: Set(signature),
            source_event_timestamp: Set(timestamp.naive_utc()),
            amount: Set(Some(event.amount)),
            event_type: Set(
                database::entities::sea_orm_active_enums::EventTypeEnum::ForcedWithdraw,
            ),
            ..Default::default()
        };

        let operation = DbOperations::BridgeSourceTransaction(model);

        Ok(operation)
    }

    async fn handle_finalize_withdrawal(
        &self,
        event: ForcedWithdrawalSuccessfulEvent,
        signature: String,
        timestamp: DateTime<Utc>,
        slot_number: u64,
    ) -> eyre::Result<DbOperations> {
        let model = bridge_destination_transactions::ActiveModel {
            source_nonce: Set(event.nonce as i64),
            source_chain_id: Set(event.chain_id as i64),
            destination_chain_id: Set(self.chain_id() as i64),
            destination_height: Set(Some(event.slot_number as i64)),
            destination_tx_hash: Set(signature),
            destination_processed_at: Set(Some(timestamp.naive_utc())),
            ..Default::default()
        };

        let operation = DbOperations::BridgeDestinationTransactions(model);
        Ok(operation)
    }
    async fn handle_finalize_l2_withdrawal(
        &self,
        event: L2WithdrawExecutedEvent,
        signature: String,
        timestamp: DateTime<Utc>,
        slot_number: u64,
    ) -> eyre::Result<DbOperations> {
        let model = bridge_destination_transactions::ActiveModel {
            source_nonce: Set(event.nonce as i64),
            source_chain_id: Set(event.chain_id as i64),
            destination_chain_id: Set(self.chain_id() as i64),
            destination_height: Set(Some(event.slot_number as i64)),
            destination_tx_hash: Set(signature),
            destination_processed_at: Set(Some(timestamp.naive_utc())),
            ..Default::default()
        };

        let operation = DbOperations::BridgeDestinationTransactions(model);
        Ok(operation)
    }

    async fn handle_commit_batch(
        &self,
        event: CommitBatchEvent,
        signature: String,
        timestamp: DateTime<Utc>,
        slot_number: u64,
    ) -> eyre::Result<DbOperations> {
        let batch_number = event.batch_number;

        let blocks = self
            .twine_provider
            .get_blocks_in_batch(batch_number)
            .await?;

        // Extract start and end block numbers from the blocks vector
        let start_block = blocks.iter().min().copied().unwrap_or(0);
        let end_block = blocks.iter().max().copied().unwrap_or(0);

        let root_hash = format!("{:?}", event.batch_hash);
        let batch_length = end_block - start_block + 1; // as both end block and start block is inclusive

        //Build batch model
        let batch_model = twine_transaction_batch::ActiveModel {
            number: Set(batch_number as i64),
            timestamp: Set(timestamp.naive_utc()),
            start_block: Set(start_block as i64),
            end_block: Set(end_block as i64),
            root_hash: Set(event.batch_hash.into()),
            ..Default::default()
        };

        let detail_model = twine_transaction_batch_detail::ActiveModel {
            batch_number: Set(batch_number as i64),
            l1_transaction_count: Set(0),
            l2_transaction_count: Set(0),
            l1_gas_price: Set(Decimal::from_f64(0.0).unwrap()),
            l2_fair_gas_price: Set(Decimal::from_f64(0.0).unwrap()),
            chain_id: Set(Decimal::from_i64(self.chain_id() as i64).unwrap()),
            commit_transaction_hash: Set(Some(signature.clone())),
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
                            am.batch_number = Set(Some(batch_number as i64));
                            am
                        })
                        .collect();

                    l2_txs = transactions
                        .into_iter()
                        .map(|model| {
                            let mut am = model.into_active_model();
                            am.batch_number = Set(Some(batch_number as i64));
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

    async fn handle_finalize_batch(
        &self,
        event: FinalizeBatchEvent,
        signature: String,
        timestamp: DateTime<Utc>,
        slot_number: u64,
    ) -> eyre::Result<DbOperations> {
        let batch_number = event.batch_number;

        Ok(DbOperations::FinalizeBatch {
            finalize_hash: signature,
            batch_number: batch_number as i64,
            chain_id: self.chain_id() as i64,
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
