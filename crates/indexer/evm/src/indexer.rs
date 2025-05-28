use std::{
    f64::consts::E,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use alloy::{primitives::U64, rpc::types::Log};
use async_trait::async_trait;
use common::config::{ChainConfig, EvmConfig};
use database::{client::DbClient, entities::last_synced, DbOperations};
use eyre::{eyre, Error};
use futures_util::StreamExt;
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

    fn get_event_handler(&self) -> Self::EventHandler {
        self.handler.clone()
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

    async fn sync_live(&self) -> Result<(), Error> {
        //TODO: Make it configurable
        const MAX_BATCH_SIZE: usize = 1000;
        const MAX_BATCH_TIME: Duration = Duration::from_secs(15);

        let mut reconnect_attempts = 0;

        loop {
            let mut last_synced = self
                .handler
                .get_db_client()
                .get_last_synced_height(self.handler.chain_id() as i64, self.config.start_block)
                .await?;

            match self
                .provider
                .subscribe_logs(
                    &self.handler.relevant_addresses(),
                    &self.handler.relevant_topics(),
                    Some(last_synced as u64),
                )
                .await
            {
                Ok(mut stream) => {
                    info!("Subscribed to live logs");
                    reconnect_attempts = 0;

                    let mut buffer = Vec::with_capacity(MAX_BATCH_SIZE);
                    let mut last_flush = Instant::now();
                    let mut max_seen_block = last_synced;

                    loop {
                        let sleep_duration = MAX_BATCH_TIME.saturating_sub(last_flush.elapsed());
                        let sleep_future = tokio::time::sleep(sleep_duration);
                        tokio::pin!(sleep_future);

                        tokio::select! {
                            maybe_log = stream.next() => {
                                match maybe_log {
                                    Some(log) => {
                                        if let Some(block_number) = log.block_number {
                                           max_seen_block = max_seen_block.max(block_number as i64);
                                        }

                                        buffer.push(log);

                                        if buffer.len() >= MAX_BATCH_SIZE  {
                                            self.process_buffer(&mut buffer, max_seen_block).await?;
                                            last_flush = Instant::now();
                                        }

                                    }
                                    None => {
                                        info!("Stream closed. Reconnecting...");
                                        break;
                                    }
                                }
                            }

                            _ = &mut sleep_future => {
                                if !buffer.is_empty() {
                                    self.process_buffer(&mut buffer, max_seen_block).await?;
                                    last_flush = Instant::now();
                                    max_seen_block = 0;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    let delay = Duration::from_secs(5);

                    reconnect_attempts += 1;
                    error!(
                        "Error subscribing to logs: {:?}. Reconnect attemp:{:?}, Retrying in {:?}",
                        e, reconnect_attempts, delay
                    );
                    tokio::time::sleep(delay * 2u32.pow(reconnect_attempts)).await;
                }
            }
        }

        Ok(())
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
