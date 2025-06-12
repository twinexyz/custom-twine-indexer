use std::{
    f64::consts::E, pin::Pin, sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    }, time::Duration
};

use alloy::{primitives::U64, rpc::types::Log};
use async_trait::async_trait;
use common::config::{ChainConfig, EvmConfig};
use database::{client::DbClient, entities::last_synced, DbOperations};
use eyre::{eyre, Error};
use futures_util::{Stream, StreamExt};
use generic_indexer::{handler::ChainEventHandler, indexer::ChainIndexer};
use tokio::{
    spawn,
    sync::{watch, Semaphore},
    task::JoinSet,
    time::Instant,
};
use tracing::{error, info, instrument};

use crate::{handler::EvmEventHandler, provider::EvmProvider};

pub struct EvmIndexer<H: EvmEventHandler + ChainEventHandler<LogType = Log>> {
    provider: EvmProvider,
    handler: H,
    config: ChainConfig,
}

#[async_trait]
impl<H: EvmEventHandler + ChainEventHandler<LogType = Log>> ChainIndexer for EvmIndexer<H> {
    type EventHandler = H;
    type LiveStream = Pin<Box<dyn Stream<Item = eyre::Result<Log>> + Send>>;

    fn get_event_handler(&self) -> Self::EventHandler {
        self.handler.clone()
    }

    async fn subscribe_live(&self, from_block: Option<u64>) -> eyre::Result<Self::LiveStream> {
        self.provider
            .subscribe_logs(
                &self.handler.relevant_addresses(),
                &self.handler.relevant_topics(),
                from_block,
            )
            .await
            .map(|stream| {
                Box::pin(stream.map(|log| Ok(log))) as Self::LiveStream
            })
    }

    async fn get_initial_state(&self) -> eyre::Result<u64> {
        let last_synced = self
            .handler
            .get_db_client()
            .get_last_synced_height(self.handler.chain_id() as i64, self.config.start_block)
            .await
            .unwrap_or(self.config.start_block as i64);
        Ok(last_synced as u64)
    }

    async fn get_current_chain_height(&self) -> eyre::Result<u64> {
        self.provider.get_block_number().await
    }

    async fn get_historical_logs(&self, from: u64, to: u64) -> eyre::Result<Vec<Log>> {
        self.provider
            .get_logs(
                &self.handler.relevant_addresses(),
                &self.handler.relevant_topics(),
                from,
                to,
            )
            .await
    }

    fn get_block_number_from_log(&self, log: &Log) -> Option<u64> {
        log.block_number
    }


}

impl<H: EvmEventHandler + ChainEventHandler<LogType = Log>> EvmIndexer<H> {
    pub async fn new(handler: H) -> Result<Self, Error> {
        let config = handler.get_chain_config();
        let provider =
            EvmProvider::new(&config.http_rpc_url, &config.ws_rpc_url, config.chain_id).await?;

        Ok(Self {
            provider,
            handler,
            config,
            // state: Arc::new(AtomicU64::new(1)),
        })
    }
}

impl<H: EvmEventHandler + ChainEventHandler<LogType = Log>> Clone for EvmIndexer<H> {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            handler: self.handler.clone(),
            config: self.config.clone(),
        }
    }
}
