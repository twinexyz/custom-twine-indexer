use super::EVMChain;
use crate::common::{poll_missing_logs, subscribe_stream, with_retry};
use crate::error::ParserError;
use crate::handler::EvmEventHandler;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::{Filter, Log};
use alloy::sol_types::SolEvent;
use async_trait::async_trait;
use common::config::TwineConfig;
use common::indexer::{MAX_RETRIES, RETRY_DELAY};
use database::client::DbClient;
use database::entities::last_synced;
use eyre::{Report, Result};
use futures_util::StreamExt;
use handlers::TwineEventHandler;
use sea_orm::ActiveValue::Set;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info};
use twine_evm_contracts::evm::twine::l2_messenger::{L2Messenger, PrecompileReturn};

pub mod handlers;

pub const TWINE_EVENT_SIGNATURES: &[&str] = &[
    L2Messenger::EthereumTransactionsHandled::SIGNATURE,
    L2Messenger::SolanaTransactionsHandled::SIGNATURE,
    L2Messenger::SentMessage::SIGNATURE,
];

pub struct TwineIndexer {
    /// WS provider for live subscription.
    ws_provider: Arc<dyn Provider + Send + Sync>,
    /// HTTP provider for polling missing blocks.
    http_provider: Arc<dyn Provider + Send + Sync>,
    db_client: Arc<DbClient>,
    chain_id: u64,
    start_block: u64,
    contract_addrs: Vec<String>,
    sync_batch_size: u64,
}

impl TwineIndexer {
    pub async fn new(config: TwineConfig, db_client: Arc<DbClient>) -> Result<Self> {
        let ws_provider =
            super::create_ws_provider(config.common.ws_rpc_url, EVMChain::Twine).await?;
        let http_provider =
            super::create_http_provider(config.common.http_rpc_url, EVMChain::Twine).await?;
        Ok(Self {
            ws_provider: Arc::new(ws_provider),
            http_provider: Arc::new(http_provider),
            db_client,
            start_block: config.common.start_block,
            chain_id: config.common.chain_id,
            contract_addrs: vec![],
            sync_batch_size: config.common.block_sync_batch_size,
        })
    }

    async fn run(&mut self) -> Result<()> {
        let id = self.chain_id();
        let last_synced = self
            .db_client
            .get_last_synced_height(self.chain_id as i64, self.start_block)
            .await?;

        let current_block = with_retry(|| async {
            self.http_provider
                .get_block_number()
                .await
                .map_err(eyre::Report::from)
        })
        .await?;

        let historical_indexer = self.clone();
        let live_indexer = self.clone();
        let sync_batch_size = self.sync_batch_size;
        let event_handler = TwineEventHandler::new(self.db_client.clone(), self.chain_id);

        let historical_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            let max_blocks_per_request = sync_batch_size;
            info!("Starting historical sync up to block {}", current_block);
            let logs = poll_missing_logs(
                &*historical_indexer.http_provider,
                last_synced as u64,
                max_blocks_per_request,
                &historical_indexer.contract_addrs,
                EVMChain::Twine,
            )
            .await?;

            historical_indexer.catchup_missing_blocks(logs).await
        });

        let live_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            info!("Starting live indexing from block {}", current_block + 1);
            let mut stream = subscribe_stream(
                &*live_indexer.ws_provider,
                &live_indexer.contract_addrs,
                EVMChain::Twine,
            )
            .await?;

            while let Some(log) = stream.next().await {
                let _ = event_handler.handle_event(log).await?;
            }
            Ok(())
        });

        let (historical_res, live_res) = tokio::join!(historical_handle, live_handle);
        historical_res??;
        live_res??;

        Ok(())
    }

    fn chain_id(&self) -> u64 {
        // self.provider.get_chain_id().await.map_err(Report::from)
        self.chain_id
    }
}

impl TwineIndexer {
    async fn catchup_missing_blocks(&self, logs: Vec<Log>) -> Result<()> {
        let id = self.chain_id();
        let event_handler = TwineEventHandler::new(self.db_client.clone(), self.chain_id);
        for log in logs {
            let _ = event_handler.handle_event(log).await;
        }
        Ok(())
    }

    fn handle_error(&self, e: Report) -> Result<()> {
        match e.downcast_ref::<ParserError>() {
            Some(ParserError::UnknownEvent { .. }) => Ok(()), // Skip unknown events
            _ => {
                tracing::error!("Error processing log: {:?}", e);
                Err(e)
            }
        }
    }
}

impl Clone for TwineIndexer {
    fn clone(&self) -> Self {
        Self {
            ws_provider: Arc::clone(&self.ws_provider),
            http_provider: Arc::clone(&self.http_provider),
            db_client: Arc::clone(&self.db_client),
            start_block: self.start_block,
            chain_id: self.chain_id,
            contract_addrs: self.contract_addrs.clone(),
            sync_batch_size: self.sync_batch_size,
        }
    }
}
