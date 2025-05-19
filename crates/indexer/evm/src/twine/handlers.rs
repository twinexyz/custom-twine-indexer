use std::{future::Future, pin::Pin};

use alloy::{
    primitives::{map::HashMap, FixedBytes, B256},
    rpc::types::Log,
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
use sea_orm::{sqlx::types::uuid::timestamp, ActiveValue::Set};
use tracing::info;
use twine_evm_contracts::evm::ethereum::{
    l1_message_queue::L1MessageQueue, twine_chain::TwineChain::CommitBatch,
};

use super::{
    parser::{DbModel, ParserError}
};

type AsyncEventHandler = Box<
    dyn Fn(&TwineEventHandler, Log) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>
        + Send
        + Sync,
>;

pub struct TwineEventHandler {
    db_client: DbClient,
    handlers: HashMap<B256, AsyncEventHandler>,
    chain_id: u64,
}

pub struct LogContext<T> {
    pub tx_hash_str: String,
    pub block_number: i64,
    pub timestamp: DateTime<Utc>,
    pub data: T,
}

impl TwineEventHandler {
    pub fn new(db_client: DbClient, chain_id: u64) -> Self {
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
    fn extract_log<T>(
        &self,
        log: Log,
        event_name: &'static str,
    ) -> eyre::Result<LogContext<T>, ParserError> {
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

        eyre::Ok(LogContext {
            tx_hash_str,
            block_number,
            timestamp,
            data: decoded.inner.data,
        });
    }



    //Handlers are here

    async fn handle_ethereum_transactions_handled(&self, log: Log) -> Result<()> {
        Ok(())
    }
}
