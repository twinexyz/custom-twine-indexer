use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use common::config::ChainConfig;
use database::client::DbClient;
use eyre::Error;
use futures_util::{stream::select_all, StreamExt};
use generic_indexer::{handler::ChainEventHandler, indexer::ChainIndexer};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta;
use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{error, info, instrument};

use crate::{handler::SolanaEventHandler, parser::FoundEvent, provider::SvmProvider};

pub struct SolanaIndexer {
    provider: SvmProvider,
    handler: SolanaEventHandler,
    max_batch_size: usize,
    db_client: Arc<DbClient>,
    config: ChainConfig,
    program_ids: Vec<Pubkey>,
}

#[async_trait]
impl ChainIndexer for SolanaIndexer {
    type EventHandler = SolanaEventHandler;

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
        self.provider.get_slot().await
    }

    async fn get_historical_logs(&self, from: u64, to: u64) -> eyre::Result<Vec<FoundEvent>> {
        self.provider
            .get_logs(self.handler.get_program_addresses(), from, to)
            .await
    }

    async fn sync_live(&self) -> eyre::Result<()> {
        let mut streams = vec![];

        for program_id in &self.program_ids {
            let stream = self.provider.subscribe_logs(program_id).await.unwrap();
            streams.push(stream);
        }

        let merged = select_all(streams);

        merged.for_each(|result| async {
            let value = result.value;
            let logs = value.logs;
            let sig = value.signature;

            // self.handler.handle_event(&logs, Some(sig)).await;
        });

        Ok(())
    }
}

impl SolanaIndexer {
    pub async fn new(db: Arc<DbClient>, handler: SolanaEventHandler) -> eyre::Result<Self> {
        let config = handler.get_chain_config();

        let provider =
            SvmProvider::new(&config.http_rpc_url, &config.ws_rpc_url, config.chain_id).await?;

        Ok(Self {
            provider,
            handler,
            max_batch_size: config.block_sync_batch_size as usize,
            db_client: db,
            config,
            program_ids: Vec::new(),
        })
    }
}
