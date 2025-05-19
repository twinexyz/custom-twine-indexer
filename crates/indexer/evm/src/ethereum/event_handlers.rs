use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use alloy::{
    primitives::{FixedBytes, B256},
    rpc::types::Log,
    sol_types::SolEvent,
};
use chrono::{DateTime, Utc};
use database::{
    client::DbClient,
    entities::{
        l1_deposit, l1_withdraw, l2_withdraw, twine_lifecycle_l1_transactions,
        twine_transaction_batch, twine_transaction_batch_detail,
    },
};
use eyre::Result;
use num_traits::FromPrimitive;
use sea_orm::{prelude::Decimal, sqlx::types::uuid::timestamp, ActiveValue::Set};
use tracing::info;
use twine_evm_contracts::evm::ethereum::{
    l1_message_queue::L1MessageQueue, twine_chain::TwineChain::CommitBatch,
};

use crate::error::ParserError;

use super::parser::{FinalizeWithdrawERC20, FinalizeWithdrawETH};

type AsyncEventHandler = Box<
    dyn Fn(&EthereumEventHandler, Log) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>
        + Send
        + Sync,
>;

pub struct EthereumEventHandler {
    db_client: Arc<DbClient>,
    handlers: HashMap<B256, AsyncEventHandler>,
    chain_id: u64,
}

pub struct LogContext<T> {
    pub tx_hash_str: String,
    pub block_number: i64,
    pub timestamp: DateTime<Utc>,
    pub data: T,
}

impl EthereumEventHandler {
    pub fn new(db_client: Arc<DbClient>, chain_id: u64) -> Self {
        Self {
            db_client,
            handlers: HashMap::new(),
            chain_id,
        }
    }

    pub fn initilize_handlers(&mut self) {
        // self.handlers.insert(
        //     L1MessageQueue::QueueDepositTransaction::SIGNATURE_HASH,
        //     Box::new(|h, log| Box::pin(h.handle_queue_deposit_txn(log))),
        // );
    }

    pub async fn handle_event(&self, log: Log) -> Result<()> {
        let sig = log.topic0().ok_or(ParserError::UnknownEvent {
            signature: B256::ZERO,
        })?;

        if let Some(handler_fn) = self.handlers.get(sig) {
            handler_fn(self, log).await
        } else {
            Err(ParserError::UnknownEvent { signature: *sig }.into())
        }
    }

    //Helper Method

    fn extract_log<T: SolEvent>(
        &self,
        log: Log,
        event_name: &'static str,
    ) -> Result<LogContext<T>, ParserError> {
        let tx_hash = log
            .transaction_hash
            .ok_or(ParserError::MissingTransactionHash)?;
        let tx_hash_str = format!("{tx_hash:?}");

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

        Ok(LogContext {
            tx_hash_str,
            block_number,
            timestamp,
            data: decoded.inner.data,
        })
    }

    // All the logs handlers are here
    async fn handle_queue_deposit_txn(&self, log: Log) -> Result<()> {
        let decoded = self.extract_log::<L1MessageQueue::QueueDepositTransaction>(
            log.clone(),
            "QueueDepositTransaction",
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

        let _ = self.db_client.insert_l1_deposits(model).await?;

        Ok(())
    }

    async fn handle_queue_withdrawal_txn(&self, log: Log) -> Result<()> {
        let decoded = self.extract_log::<L1MessageQueue::QueueWithdrawalTransaction>(
            log.clone(),
            "QueueWithdrawalTransaction",
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
        let decoded =
            self.extract_log::<FinalizeWithdrawERC20>(log.clone(), "FinalizeWithdrawERC20")?;
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
            self.extract_log::<FinalizeWithdrawETH>(log.clone(), "FinalizeWithdrawETH")?;
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
        let decoded = self.extract_log::<CommitBatch>(log.clone(), "CommitBatch")?;

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
            }
        }

        let _ =
            self.db_client
                .commit_batch(batch_model, detail_model, lifecyle_txn, l2_blocks, l2_txs);

        Ok(())
    }
}
