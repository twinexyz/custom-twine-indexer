use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use alloy::{
    primitives::{FixedBytes, B256},
    rpc::types::Log,
    sol_types::SolEvent,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::config::{ChainConfig, EvmConfig};
use database::{
    blockscout_entities::{
        blocks, transactions, twine_transaction_batch, twine_transaction_batch_detail,
    },
    client::DbClient,
    entities::{bridge_destination_transactions, bridge_source_transactions},
    DbOperations,
};
use eyre::Result;
use generic_indexer::handler::ChainEventHandler;
use num_traits::FromPrimitive;
use sea_orm::{prelude::Decimal, sqlx::types::uuid::timestamp, ActiveValue::Set, IntoActiveModel};
use tracing::{error, info, instrument, warn};
use twine_evm_contracts::evm::ethereum::{
    l1_message_handler::L1MessageHandler,
    twine_chain::TwineChain::{self, CommitedBatch, FinalizedBatch},
};

use crate::{
    error::ParserError,
    ethereum::parser::get_event_name_from_signature_hash,
    handler::{EvmEventHandler, LogContext},
    provider::EvmProvider,
    EVMChain,
};

use super::ETHEREUM_EVENT_SIGNATURES;

#[derive(Clone)]
pub struct EthereumEventHandler {
    db_client: Arc<DbClient>,
    chain_id: u64,
    config: EvmConfig,
    twine_provider: Arc<EvmProvider>,
}

#[async_trait]
impl ChainEventHandler for EthereumEventHandler {
    type LogType = Log;

    #[instrument(skip_all, fields(CHAIN = "Ethereum"))]
    async fn handle_event(&self, log: Log) -> eyre::Result<Vec<DbOperations>> {
        let sig = log.topic0().ok_or(ParserError::UnknownEvent {
            signature: B256::ZERO,
        })?;
        let block_number = log.block_number.unwrap();

        let mut operations = Vec::new();

        info!(
            "Received event '{}' in block {} (signature hash: 0x{})",
            get_event_name_from_signature_hash(sig),
            block_number,
            hex::encode(sig),
        );

        match *sig {
            L1MessageHandler::MessageTransaction::SIGNATURE_HASH => {
                let operation = self.handle_l1_message_transaction(log).await?;
                operations.push(operation);
            }
            TwineChain::L2WithdrawExecuted::SIGNATURE_HASH => {
                let operation = self.handle_finalize_withdraw(log).await?;
                operations.push(operation);
            }

            // TwineChain::RefundSuccessful::SIGNATURE_HASH => {
            //     let operation = self.handle_finalize_withdraw_eth(log).await?;
            //     operations.push(operation);
            // }

            // TwineChain::ForcedWithdrawalSuccessful::SIGNATURE_HASH => {
            //     let operation = self.handle_finalize_withdraw(log).await?;
            //     operations.push(operation);
            // }
            CommitedBatch::SIGNATURE_HASH => {
                let operation = self.handle_commit_batch(log).await?;
                operations.push(operation);
            }

            FinalizedBatch::SIGNATURE_HASH => {
                let operation = self.handle_finalize_batch(log).await?;
                operations.push(operation);
            }
            other => {
                error!("Unknown event to handle")
            }
        }

        Ok(operations)
    }

    fn get_chain_config(&self) -> common::config::ChainConfig {
        self.config.common.clone()
    }
}

#[async_trait]
impl EvmEventHandler for EthereumEventHandler {
    fn relevant_addresses(&self) -> Vec<alloy::primitives::Address> {
        let addresss = [
            self.config.l1_erc20_gateway_address.clone(),
            self.config.eth_twine_chain_address.clone(),
            self.config.l1_message_queue_address.clone(),
        ];

        let contract_addresss = addresss
            .iter()
            .map(|addr| addr.parse::<alloy::primitives::Address>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| eyre::eyre!("Invalid address format: {}", e))
            .unwrap();

        contract_addresss
    }

    fn relevant_topics(&self) -> Vec<&'static str> {
        ETHEREUM_EVENT_SIGNATURES.to_vec()
    }
}

impl EthereumEventHandler {
    pub fn new(
        db_client: Arc<DbClient>,
        config: EvmConfig,
        twine_provider: Arc<EvmProvider>,
    ) -> Self {
        Self {
            db_client,
            chain_id: config.common.chain_id,
            config,
            twine_provider,
        }
    }

    async fn handle_l1_message_transaction(&self, log: Log) -> Result<DbOperations> {
        let decoded = self.extract_log::<L1MessageHandler::MessageTransaction>(
            log.clone(),
            L1MessageHandler::MessageTransaction::SIGNATURE,
        )?;

        let data = decoded.data;

        match data.txnType {
            L1MessageHandler::TransactionType::Deposit => self.handle_queue_deposit_txn(log).await,
            L1MessageHandler::TransactionType::Withdraw => {
                self.handle_queue_withdrawal_txn(log).await
            }
            _ => return Err(eyre::eyre!("Invalid transaction type")),
        }
    }

    // All the logs handlers are here
    async fn handle_queue_deposit_txn(&self, log: Log) -> Result<DbOperations> {
        let decoded = self.extract_log::<L1MessageHandler::MessageTransaction>(
            log.clone(),
            L1MessageHandler::MessageTransaction::SIGNATURE,
        )?;

        let data = decoded.data;

        let model = bridge_source_transactions::ActiveModel {
            source_nonce: Set(data.nonce.try_into().unwrap()),
            source_chain_id: Set(data.chainId.try_into().unwrap()),
            source_height: Set(Some(data.blockNumber.try_into().unwrap())),
            source_from_address: Set(format!("{:?}", data.l1Address)),
            target_recipient_address: Set(Some(format!("{:?}", data.twineAddress))),
            destination_token_address: Set(Some(format!("{:?}", data.l2Token))),
            source_token_address: Set(Some(format!("{:?}", data.l1Token))),
            source_tx_hash: Set(decoded.tx_hash_str.clone()),
            source_event_timestamp: Set(decoded.timestamp.naive_utc()),
            amount: Set(Some(data.amount.to_string())),
            event_type: Set(database::entities::sea_orm_active_enums::EventTypeEnum::Deposit),
            ..Default::default()
        };

        let operation = DbOperations::BridgeSourceTransaction(model);

        // let _ = self.db_client.insert_l1_deposits(model).await?;

        Ok(operation)
    }

    async fn handle_queue_withdrawal_txn(&self, log: Log) -> Result<DbOperations> {
        let decoded = self.extract_log::<L1MessageHandler::MessageTransaction>(
            log.clone(),
            L1MessageHandler::MessageTransaction::SIGNATURE,
        )?;
        let data = decoded.data;

        let model = bridge_source_transactions::ActiveModel {
            source_nonce: Set(data.nonce.try_into().unwrap()),
            source_chain_id: Set(data.chainId.try_into().unwrap()),
            source_height: Set(Some(data.blockNumber.try_into().unwrap())),
            source_from_address: Set(format!("{:?}", data.l1Address)),
            target_recipient_address: Set(Some(format!("{:?}", data.twineAddress))),
            destination_token_address: Set(Some(format!("{:?}", data.l2Token))),
            source_token_address: Set(Some(format!("{:?}", data.l1Token))),
            source_tx_hash: Set(decoded.tx_hash_str.clone()),
            source_event_timestamp: Set(decoded.timestamp.naive_utc()),
            amount: Set(Some(data.amount.to_string())),
            event_type: Set(
                database::entities::sea_orm_active_enums::EventTypeEnum::ForcedWithdraw,
            ),
            ..Default::default()
        };

        let operation = DbOperations::BridgeSourceTransaction(model);

        // let _ = self.db_client.insert_l1_withdraw(model).await?;

        Ok(operation)
    }

    async fn handle_finalize_withdraw(&self, log: Log) -> Result<DbOperations> {
        let decoded = self.extract_log::<TwineChain::L2WithdrawExecuted>(
            log.clone(),
            TwineChain::L2WithdrawExecuted::SIGNATURE,
        )?;
        let data = decoded.data;

        let l2_chain_id = self.twine_provider.get_chain_id();

        let model = bridge_destination_transactions::ActiveModel {
            source_nonce: Set(data.nonce.try_into().unwrap()),
            source_chain_id: Set(l2_chain_id as i64),
            destination_chain_id: Set(self.chain_id as i64),
            destination_height: Set(Some(data.blockNumber.try_into().unwrap())),
            destination_tx_hash: Set(decoded.tx_hash_str.clone()),
            destination_processed_at: Set(Some(decoded.timestamp.naive_utc())),
            ..Default::default()
        };
        // let _ = self.db_client.insert_l2_withdraw(model).await?;
        let operation = DbOperations::BridgeDestinationTransactions(model);
        Ok(operation)
    }

    async fn handle_commit_batch(&self, log: Log) -> Result<DbOperations> {
        let decoded = self.extract_log::<CommitedBatch>(log.clone(), CommitedBatch::SIGNATURE)?;

        let data = decoded.data;

        let batch_number = data.batchNumber;
        let blocks = self
            .twine_provider
            .get_blocks_in_batch(batch_number)
            .await?;

        // Extract start and end block numbers from the blocks vector
        let start_block = blocks.iter().min().copied().unwrap_or(0);
        let end_block = blocks.iter().max().copied().unwrap_or(0);

        let root_hash = format!("{:?}", data.batchHash);

        //Build Batch Model
        let batch_model = twine_transaction_batch::ActiveModel {
            number: Set(batch_number as i64),
            timestamp: Set(decoded.timestamp.naive_utc()),
            start_block: Set(start_block as i64),
            end_block: Set(end_block as i64),
            root_hash: Set(alloy::hex::decode(root_hash.trim_start_matches("0x")).unwrap()),
            ..Default::default()
        };

        // Build Batch Details Table
        let detail_model = twine_transaction_batch_detail::ActiveModel {
            batch_number: Set(batch_number as i64),
            l1_transaction_count: Set(0),
            l2_transaction_count: Set(0),
            l1_gas_price: Set(Decimal::from_f64(0.0).unwrap()),
            l2_fair_gas_price: Set(Decimal::from_f64(0.0).unwrap()),
            chain_id: Set(Decimal::from_i64(self.chain_id as i64).unwrap()),
            commit_transaction_hash: Set(Some(decoded.tx_hash_str.clone().into_bytes())),
            finalize_transaction_hash: Set(None),
            ..Default::default()
        };

        let mut l2_blocks = Vec::new();
        let mut l2_txs = Vec::new();

        //1. Check if batch already exists
        match self.db_client.get_batch_by_id(batch_number as i64).await? {
            Some(existing_batch) => {
                //If the batch already exists, it means it already has its corresponding transactions and blocks as welll.
                // So we just need to put twine commit information in the table

                info!("Batch already exists so don't need to fetch blocks and transactions");
            }
            None => {
                // info!("First time encoutering this batch, so need to fecth blocks and transactions from twine");

                let blocks = self.db_client.get_blocks(start_block, end_block).await?;

                let batch_length = end_block - start_block + 1;

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

    async fn handle_finalize_batch(&self, log: Log) -> Result<DbOperations> {
        let decoded = self.extract_log::<FinalizedBatch>(log.clone(), FinalizedBatch::SIGNATURE)?;

        let batch_number = decoded.data.batchNumber;
        let tx_hash_bytes = decoded.tx_hash_str.clone().into_bytes();
        let timestamp = decoded.timestamp.naive_utc();
        let chain_id_dec = Decimal::from_i64(self.chain_id as i64).unwrap();

        let operation = DbOperations::FinalizeBatch {
            finalize_hash: tx_hash_bytes,
            batch_number: batch_number as i64,
        };

        Ok(operation)
    }
}
