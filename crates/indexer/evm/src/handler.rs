use std::{collections::HashMap, future::Future, pin::Pin};

use alloy::{
    primitives::{Address, B256},
    rpc::types::Log,
    sol_types::SolEvent,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use common::config::ChainConfig;
use database::{client::DbClient, DbOperations};
use eyre::Result;

use crate::error::ParserError;

type AsyncEventHandler = Box<
    dyn Fn(&dyn EvmEventHandler, Log) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>
        + Send
        + Sync,
>;

pub struct LogContext<T> {
    pub tx_hash_str: String,
    pub block_number: i64,
    pub timestamp: DateTime<Utc>,
    pub data: T,
}

#[async_trait]
pub trait EvmEventHandler: Send + Sync + Clone + 'static {
    fn chain_id(&self) -> u64;
    fn get_chain_config(&self) -> ChainConfig;

    fn relevant_addresses(&self) -> Vec<Address>;

    fn relevant_topics(&self) -> Vec<&'static str>;

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

    async fn handle_event(&self, log: Log) -> Result<Vec<DbOperations>>;
}
