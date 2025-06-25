use std::{pin::Pin, sync::Arc, time::Duration};

use async_trait::async_trait;
use common::config::{ChainConfig, IndexerSettings};
use database::client::DbClient;
use eyre::Error;
use futures_util::{stream::select_all, Stream, StreamExt};
use generic_indexer::{handler::ChainEventHandler, indexer::ChainIndexer};
use solana_client::rpc_response::{Response, RpcLogsResponse};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta;
use tokio::{sync::Semaphore, task::JoinSet};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info, instrument};

use crate::{
    handler::SolanaEventHandler,
    parser::{parse_log, SolanaLog},
    provider::SvmProvider,
};

pub struct SolanaIndexer {
    provider: SvmProvider,
    handler: SolanaEventHandler,
    max_batch_size: usize,
    db_client: Arc<DbClient>,
    config: ChainConfig,
    settings: IndexerSettings,
}

#[async_trait]
impl ChainIndexer for SolanaIndexer {
    type EventHandler = SolanaEventHandler;
    type LiveStream = Pin<Box<dyn Stream<Item = eyre::Result<SolanaLog>> + Send>>;

    fn get_event_handler(&self) -> Self::EventHandler {
        self.handler.clone()
    }
    fn get_db_client(&self) -> Arc<DbClient> {
        self.db_client.clone()
    }
    fn get_indexer_settings(&self) -> IndexerSettings {
        self.settings.clone()
    }

    async fn subscribe_live(&self, _from_block: Option<u64>) -> eyre::Result<Self::LiveStream> {
        let program_ids = self.handler.get_program_addresses();

        let mut streams = Vec::new();

        for program_id in program_ids.clone() {
            let raw_stream = self.provider.subscribe_logs(&program_id).await?;

            streams.push(raw_stream);
        }

        if streams.is_empty() && !program_ids.is_empty() {
            return Err(eyre::eyre!(
                "Solana: No log streams could be established despite configured program IDs."
            ));
        } else if streams.is_empty() {
            return Ok(futures_util::stream::empty().boxed());
        } else {
            let merged = select_all(streams).boxed();
            let parsed_stream = merged.map(move |result| parse_log(result));

            return Ok(Box::pin(parsed_stream));
        }
    }

    async fn get_initial_state(&self) -> eyre::Result<u64> {
        let last_synced = self
            .db_client
            .get_last_synced_height(self.handler.chain_id() as i64, self.config.start_block)
            .await
            .unwrap_or(self.config.start_block as i64);
        Ok(last_synced as u64)
    }

    async fn get_current_chain_height(&self) -> eyre::Result<u64> {
        self.provider.get_slot().await
    }

    async fn get_historical_logs(&self, from: u64, to: u64) -> eyre::Result<Vec<SolanaLog>> {
        self.provider
            .get_logs(self.handler.get_program_addresses(), from, to)
            .await
    }

    fn get_block_number_from_log(&self, log: &SolanaLog) -> Option<u64> {
        Some(log.slot)
    }
}

impl SolanaIndexer {
    pub fn new(handler: SolanaEventHandler, db: Arc<DbClient>, settings: IndexerSettings) -> Self {
        let config = handler.get_chain_config();

        let provider = SvmProvider::new(&config.http_rpc_url, &config.ws_rpc_url, config.chain_id);

        Self {
            provider,
            handler,
            max_batch_size: config.block_sync_batch_size as usize,
            db_client: db,
            config,
            settings,
        }
    }
}
