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
    client::DbClient,
    entities::{
        l1_deposit, l1_withdraw, l2_withdraw, twine_batch_l2_blocks, twine_batch_l2_transactions,
        twine_lifecycle_l1_transactions, twine_transaction_batch, twine_transaction_batch_detail,
    },
};
use eyre::Result;
use num_traits::FromPrimitive;
use sea_orm::{prelude::Decimal, sqlx::types::uuid::timestamp, ActiveValue::Set};
use tracing::{error, info, instrument, warn};
use twine_evm_contracts::evm::ethereum::{
    l1_message_queue::L1MessageQueue,
    twine_chain::TwineChain::{CommitBatch, FinalizedBatch},
};

use crate::{error::ParserError, handler::EvmEventHandler, provider::EvmProvider, EVMChain};

use super::{
    parser::{FinalizeWithdrawERC20, FinalizeWithdrawETH},
    ETHEREUM_EVENT_SIGNATURES,
};

pub struct EthereumEventHandler {
    db_client: Arc<DbClient>,
    chain_id: u64,
    config: EvmConfig,
    twine_provider: EvmProvider,
}

#[async_trait]
impl EvmEventHandler for EthereumEventHandler {
    #[instrument(skip_all, fields(CHAIN = "Ethereum"))]
    async fn handle_event(&self, log: Log) -> eyre::Result<()> {
        let sig = log.topic0().ok_or(ParserError::UnknownEvent {
            signature: B256::ZERO,
        })?;
        let block_number = log.block_number.unwrap();


        match *sig {
            L1MessageQueue::QueueDepositTransaction::SIGNATURE_HASH => {
                let processed = self.handle_queue_deposit_txn(log).await;
            }
            L1MessageQueue::QueueWithdrawalTransaction::SIGNATURE_HASH => {
                let processed = self.handle_queue_withdrawal_txn(log).await?;
            }
            FinalizeWithdrawERC20::SIGNATURE_HASH => {
                let processed = self.handle_finalize_withdraw(log).await?;
            }

            FinalizeWithdrawETH::SIGNATURE_HASH => {
                let processed = self.handle_finalize_withdraw_eth(log).await?;
            }

            CommitBatch::SIGNATURE_HASH => {
                let procesesd = self.handle_commit_batch(log).await?;
            }

            FinalizedBatch::SIGNATURE_HASH => {
                let processed = self.handle_finalize_batch(log).await?;
            }
            other => {
                error!("Unknown event to handle")
            }
        }

        //if proccessd block number exists or greater than 0, update last synced table

        info!("Proccessed block: {}", block_number);

        self.db_client
            .upsert_last_synced(self.chain_id as i64, block_number as i64)
            .await?;

        Ok(())
    }

    fn chain_id(&self) -> u64 {
        self.chain_id
    }

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
    fn get_chain_config(&self) -> common::config::ChainConfig {
        self.config.common.clone()
    }

    fn extract_log<T: SolEvent>(
        &self,
        log: Log,
        event_name: &'static str,
    ) -> Result<crate::handler::LogContext<T>, ParserError> {
        let tx_hash = log
            .transaction_hash
            .ok_or(ParserError::MissingTransactionHash)?;
        let tx_hash_str = std::format!("{tx_hash:?}");

        let block_number = log.block_number.ok_or(ParserError::MissingBlockNumber)? as i64;

        let timestamp = log
            .block_timestamp
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts as i64, 0))
            .unwrap_or_else(|| {
                tracing::warn!(
                    "Missing or invalid block timestamp in {}. Using now.",
                    event_name
                );
                Utc::now()
            });

        let decoded = log
            .log_decode::<T>()
            .map_err(|e| ParserError::DecodeError {
                event_type: event_name,
                source: Box::new(e),
            })?;

        Ok(crate::handler::LogContext {
            tx_hash_str,
            block_number,
            timestamp,
            data: decoded.inner.data,
        })
    }
}

impl EthereumEventHandler {
    pub fn new(db_client: Arc<DbClient>, config: EvmConfig, twine_provider: EvmProvider) -> Self {
        Self {
            db_client,
            chain_id: config.common.chain_id,
            config,
            twine_provider,
        }
    }

    // All the logs handlers are here
    async fn handle_queue_deposit_txn(&self, log: Log) -> Result<()> {
        let decoded = self.extract_log::<L1MessageQueue::QueueDepositTransaction>(
            log.clone(),
            L1MessageQueue::QueueDepositTransaction::SIGNATURE,
        )?;

        let data = decoded.data;
        let model = l1_deposit::ActiveModel {
            nonce: Set(data.nonce.try_into().unwrap()),
            chain_id: Set(data.chainId.try_into().unwrap()),
            block_number: Set(Some(data.blockNumber.try_into().unwrap())),
            slot_number: Set(None),
            from: Set(format!("{:?}", data.from)),
            to_twine_address: Set(format!("{:?}", data.toTwineAddress)),
            l1_token: Set(format!("{:?}", data.l1Token)),
            l2_token: Set(format!("{:?}", data.l2Token)),
            tx_hash: Set(decoded.tx_hash_str),
            amount: Set(data.amount.to_string()),
            created_at: Set(decoded.timestamp.into()),
        };

        // let _ = self.db_client.insert_l1_deposits(model).await?;

        Ok(())
    }

    async fn handle_queue_withdrawal_txn(&self, log: Log) -> Result<()> {
        let decoded = self.extract_log::<L1MessageQueue::QueueWithdrawalTransaction>(
            log.clone(),
            L1MessageQueue::QueueWithdrawalTransaction::SIGNATURE,
        )?;

        let model = l1_withdraw::ActiveModel {
            tx_hash: Set(decoded.tx_hash_str.clone()),
            nonce: Set(decoded.data.nonce.try_into().unwrap()),
            chain_id: Set(decoded.data.chainId.try_into().unwrap()),
            block_number: Set(Some(decoded.data.blockNumber.try_into().unwrap())),
            slot_number: Set(None),
            l1_token: Set(format!("{:?}", decoded.data.l1Token)),
            l2_token: Set(format!("{:?}", decoded.data.l2Token)),
            from: Set(format!("{:?}", decoded.data.from)),
            to_twine_address: Set(format!("{:?}", decoded.data.toTwineAddress)),
            amount: Set(decoded.data.amount.to_string()),
            created_at: Set(decoded.timestamp.into()),
        };

        let _ = self.db_client.insert_l1_withdraw(model).await?;

        Ok(())
    }

    async fn handle_finalize_withdraw(&self, log: Log) -> Result<()> {
        let decoded = self
            .extract_log::<FinalizeWithdrawERC20>(log.clone(), FinalizeWithdrawERC20::SIGNATURE)?;
        let data = decoded.data;

        let model = l2_withdraw::ActiveModel {
            chain_id: Set(data.chainId.try_into().unwrap()),
            nonce: Set(data.nonce.try_into().unwrap()),
            block_number: Set(Some(data.blockNumber.try_into().unwrap())),
            slot_number: Set(None),
            tx_hash: Set(decoded.tx_hash_str.clone()),
            created_at: Set(decoded.timestamp.into()),
        };
        let _ = self.db_client.insert_l2_withdraw(model).await?;

        Ok(())
    }

    async fn handle_finalize_withdraw_eth(&self, log: Log) -> Result<()> {
        let decoded =
            self.extract_log::<FinalizeWithdrawETH>(log.clone(), FinalizeWithdrawETH::SIGNATURE)?;
        let data = decoded.data;
        let model = l2_withdraw::ActiveModel {
            chain_id: Set(data.chainId.try_into().unwrap()),
            nonce: Set(data.nonce.try_into().unwrap()),
            block_number: Set(Some(data.blockNumber.try_into().unwrap())),
            slot_number: Set(None),
            tx_hash: Set(decoded.tx_hash_str.clone()),
            created_at: Set(decoded.timestamp.into()),
        };
        let _ = self.db_client.insert_l2_withdraw(model).await?;

        Ok(())
    }

    async fn handle_commit_batch(&self, log: Log) -> Result<()> {
        let decoded = self.extract_log::<CommitBatch>(log.clone(), CommitBatch::SIGNATURE)?;

        let data = decoded.data;

        let start_block = data.startBlock;
        let end_block = data.endBlock;
        let root_hash = format!("{:?}", data.batchHash);

        //Build Batch Model
        let batch_model = twine_transaction_batch::ActiveModel {
            number: Set(start_block as i64),
            timestamp: Set(decoded.timestamp.naive_utc()),
            start_block: Set(start_block as i64),
            end_block: Set(end_block as i64),
            root_hash: Set(alloy::hex::decode(root_hash.trim_start_matches("0x")).unwrap()),
            ..Default::default()
        };

        // Build Lifecycle Transation Model
        let lifecyle_txn = twine_lifecycle_l1_transactions::ActiveModel {
            hash: Set(decoded.tx_hash_str.clone().into_bytes()),
            chain_id: Set(Decimal::from_i64(self.chain_id as i64).unwrap()),
            timestamp: Set(decoded.timestamp.naive_utc()),
            ..Default::default()
        };

        // Build Batch Details Table
        let detail_model = twine_transaction_batch_detail::ActiveModel {
            batch_number: Set(start_block as i64),
            l1_transaction_count: Set(0),
            l2_transaction_count: Set(0),
            l1_gas_price: Set(Decimal::from_f64(0.0).unwrap()),
            l2_fair_gas_price: Set(Decimal::from_f64(0.0).unwrap()),
            chain_id: Set(Decimal::from_i64(self.chain_id as i64).unwrap()),
            commit_id: Set(None),
            execute_id: Set(None),
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
                info!("First time encoutering this batch, so need to fecth blocks and transactions from twine");

                let (blocks, transactions) = self
                    .twine_provider
                    .get_blocks_with_transactions(start_block, end_block)
                    .await?;

                for block in blocks {
                    l2_blocks.push(twine_batch_l2_blocks::ActiveModel {
                        batch_number: Set(start_block as i64),
                        hash: Set(block.header.hash.to_vec()),
                        ..Default::default()
                    });
                }

                for transaction in transactions {
                    l2_txs.push(twine_batch_l2_transactions::ActiveModel {
                        batch_number: Set(start_block as i64),
                        hash: Set(transaction.inner.tx_hash().to_vec()),
                        ..Default::default()
                    });
                }
            }
        }

        let _ = self
            .db_client
            .commit_batch(batch_model, detail_model, lifecyle_txn, l2_blocks, l2_txs)
            .await;

        Ok(())
    }

    async fn handle_finalize_batch(&self, log: Log) -> Result<()> {
        let decoded = self.extract_log::<FinalizedBatch>(log.clone(), FinalizedBatch::SIGNATURE)?;
        let start_block = decoded.data.startBlock as i64;
        let tx_hash_bytes = decoded.tx_hash_str.clone().into_bytes();
        let timestamp = decoded.timestamp.naive_utc();
        let chain_id_dec = Decimal::from_i64(self.chain_id as i64).unwrap();

        // Fetch existing batch details once
        if let Some(existing) = self.db_client.get_batch_details(start_block).await? {
            // Prepare lifecycle transaction model
            let lifecycle_txn = twine_lifecycle_l1_transactions::ActiveModel {
                hash: Set(tx_hash_bytes),
                chain_id: Set(chain_id_dec.clone()),
                timestamp: Set(timestamp),
                ..Default::default()
            };

            // Prepare updated batch detail model using existing values
            let detail_model = twine_transaction_batch_detail::ActiveModel {
                batch_number: Set(start_block),
                l1_transaction_count: Set(existing.l1_transaction_count),
                l2_transaction_count: Set(existing.l2_transaction_count),
                l1_gas_price: Set(Decimal::from_f64(0.0).unwrap()),
                l2_fair_gas_price: Set(Decimal::from_f64(0.0).unwrap()),
                chain_id: Set(chain_id_dec),
                commit_id: Set(existing.commit_id),
                id: Set(existing.id),
                execute_id: Set(None), //will be set by db service
                ..Default::default()
            };

            // Apply the finalize_batch update
            self.db_client
                .finalize_batch(detail_model, lifecycle_txn)
                .await?;
        } else {
            warn!("Finalize Event indexed before commitment. Skipping");
        }

        Ok(())
    }
}

impl Clone for EthereumEventHandler {
    fn clone(&self) -> Self {
        Self {
            chain_id: self.chain_id,
            config: self.config.clone(),
            db_client: self.db_client.clone(),
            twine_provider: self.twine_provider.clone(),
        }
    }
}
