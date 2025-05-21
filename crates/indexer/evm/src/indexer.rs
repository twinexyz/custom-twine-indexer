use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use alloy::{primitives::U64, rpc::types::Log};
use common::config::{ChainConfig, EvmConfig};
use database::client::DbClient;
use eyre::Error;
use futures_util::StreamExt;
use tokio::{spawn, sync::watch};

use crate::{handler::EvmEventHandler, provider::EvmProvider};

pub struct EvmIndexer<H: EvmEventHandler> {
    provider: EvmProvider,
    handler: H,
    db: Arc<DbClient>,
    config: ChainConfig,
    state: Arc<AtomicU64>,
}

impl<H: EvmEventHandler> EvmIndexer<H> {
    pub async fn new(handler: H, db: Arc<DbClient>) -> Result<Self, Error> {
        let config = handler.get_chain_config();
        let provider =
            EvmProvider::new(&config.http_rpc_url, &config.ws_rpc_url, config.chain_id).await?;

        Ok(Self {
            provider,
            handler,
            db,
            config,
            state: Arc::new(AtomicU64::new(1)),
        })
    }

    pub async fn run(&mut self) -> Result<(), Error> {
        let initial_block = self.initialize_state().await?;

        let (tx, rx) = watch::channel(initial_block);

        let hist_indexer = self.clone();
        let live_indexer = self.clone();

        let historical_handle = tokio::spawn(async move {
            hist_indexer
                .sync_historical(hist_indexer.state.clone(), tx)
                .await
        });

        let live_handle = tokio::spawn(async move {
            let mut rx = rx.clone();
            live_indexer
                .sync_live(live_indexer.state.clone(), &mut rx)
                .await
        });

        // Wait for both tasks to complete
        let (historical_res, live_res) = tokio::join!(historical_handle, live_handle);

        historical_res??;
        live_res??;

        Ok(())
    }

    async fn initialize_state(&self) -> Result<u64, Error> {
        let last_synced = self
            .db
            .get_last_synced_height(self.handler.chain_id() as i64, self.config.start_block)
            .await
            .unwrap_or(self.config.start_block as i64);

        self.state.store(last_synced as u64, Ordering::Release);

        Ok(last_synced as u64)
    }

    async fn sync_historical(
        &self,
        state: Arc<AtomicU64>,
        block_sender: watch::Sender<u64>,
    ) -> Result<(), Error> {
        let mut current_state_block = state.load(Ordering::Acquire);
        let current_block = self.provider.get_block_number().await?;

        while current_state_block < current_block {
            let end_block =
                (current_state_block + self.config.block_sync_batch_size).min(current_block);

            let logs = self
                .provider
                .get_logs(
                    &self.handler.relevant_addresses(),
                    &self.handler.relevant_topics(),
                    current_state_block,
                    end_block,
                )
                .await?;

            self.process_logs(logs).await?;

            current_state_block = end_block + 1;

            state.store(current_state_block, Ordering::Release);
            block_sender.send(current_block)?;

            self.db
                .upsert_last_synced(self.handler.chain_id() as i64, current_state_block as i64)
                .await?;
        }
        Ok(())
    }

    async fn sync_live(
        &self,
        state: Arc<AtomicU64>,
        block_receiver: &mut watch::Receiver<u64>,
    ) -> Result<(), Error> {
        let mut current_state_block = state.load(Ordering::Acquire);

        let mut stream = self
            .provider
            .subscribe_logs(
                &self.handler.relevant_addresses(),
                &self.handler.relevant_topics(),
            )
            .await?;

        while let Some(log_result) = stream.next().await {
            let log = log_result;

            self.handler.handle_event(log.clone()).await?;

            if let Some(block_num) = log.block_number {
                if block_num > current_state_block {
                    current_state_block = block_num;
                    self.db
                        .upsert_last_synced(
                            self.handler.chain_id() as i64,
                            current_state_block as i64,
                        )
                        .await?;
                }
            }
        }
        Ok(())
    }

    async fn process_logs(&self, logs: Vec<Log>) -> Result<(), Error> {
        for log in logs {
            self.handler.handle_event(log.clone()).await?;
        }
        Ok(())
    }
}

impl<H: EvmEventHandler> Clone for EvmIndexer<H> {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            handler: self.handler.clone(),
            db: self.db.clone(),
            config: self.config.clone(),
            state: self.state.clone(),
        }
    }
}
