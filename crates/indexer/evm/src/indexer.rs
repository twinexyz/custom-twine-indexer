use std::{
    f64::consts::E,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use alloy::{primitives::U64, rpc::types::Log};
use async_trait::async_trait;
use common::config::{ChainConfig, EvmConfig, IndexerSettings};
use database::{client::DbClient, entities::last_synced, DbOperations};
use eyre::{eyre, Error};
use futures_util::{Stream, StreamExt};
use generic_indexer::{handler::ChainEventHandler, indexer::ChainIndexer, state::IndexerState};
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
    db_client: Arc<DbClient>,
    settings: IndexerSettings,
}

#[async_trait]
impl<H: EvmEventHandler + ChainEventHandler<LogType = Log>> ChainIndexer for EvmIndexer<H> {
    type EventHandler = H;

    fn get_event_handler(&self) -> Self::EventHandler {
        self.handler.clone()
    }

    fn get_db_client(&self) -> Arc<DbClient> {
        self.db_client.clone()
    }

    fn get_indexer_settings(&self) -> IndexerSettings {
        self.settings.clone()
    }

    async fn get_initial_state(&self) -> eyre::Result<u64> {
        let last_synced = self
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
                &self.handler.relevant_addresses().await,
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
    pub fn new(handler: H, db_client: Arc<DbClient>, settings: IndexerSettings) -> Self {
        let config = handler.get_chain_config();
        let provider = EvmProvider::new(&config.http_rpc_url, config.chain_id);

        Self {
            provider,
            handler,
            config,
            db_client,
            settings,
        }
    }
}

impl<H: EvmEventHandler + ChainEventHandler<LogType = Log>> Clone for EvmIndexer<H> {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            handler: self.handler.clone(),
            config: self.config.clone(),
            db_client: self.db_client.clone(),
            settings: self.settings.clone(),
        }
    }
}
