mod parser;

use super::EVMChain;
use crate::common::{
    create_http_provider, create_ws_provider, poll_missing_logs, subscribe_stream, with_retry,
};
use crate::error::ParserError;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Log;
use alloy::sol;
use alloy::sol_types::SolEvent;
use async_trait::async_trait;
use common::config::EvmConfig;
use common::indexer::{MAX_RETRIES, RETRY_DELAY};
use database::client::DbClient;
use database::entities::last_synced;
use event_handlers::EthereumEventHandler;
use eyre::{Report, Result};
use futures_util::{future, StreamExt};
use providers::evm::EVMProvider;
use providers::ChainProvider;
use sea_orm::ActiveValue::Set;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info};
use twine_evm_contracts::evm::ethereum::l1_message_queue::L1MessageQueue;
use twine_evm_contracts::evm::ethereum::twine_chain::TwineChain::{
    CommitBatch, FinalizedBatch, FinalizedTransaction,
};

pub mod event_handlers;

pub struct EthereumIndexer {
    chain_id: u64,
    start_block: u64,
    contract_addrs: Vec<String>,
    db_client: Arc<DbClient>,
    /// WS provider for live subscription.
    ws_provider: Arc<dyn Provider + Send + Sync>,
    /// HTTP provider for polling missing blocks.
    http_provider: Arc<dyn Provider + Send + Sync>,
}

impl EthereumIndexer {
    pub async fn new(config: EvmConfig, db_client: Arc<DbClient>) -> Result<Self> {
        let http_provider =
            create_http_provider(config.common.http_rpc_url, EVMChain::Ethereum).await?;
        let ws_provider = create_ws_provider(config.common.ws_rpc_url, EVMChain::Ethereum).await?;

        Ok(Self {
            http_provider: Arc::new(http_provider),
            ws_provider: Arc::new(ws_provider),
            chain_id: config.common.chain_id,
            start_block: config.common.start_block,
            contract_addrs: vec![],
            db_client,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
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

        let event_handler =
            EthereumEventHandler::new(self.db_client.clone(), self.chain_id.clone());

        let historical_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            let max_blocks_per_request = std::env::var("ETHEREUM__BLOCK_SYNC_BATCH_SIZE")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(100);

            info!("Starting historical sync up to block {}", current_block);
            let logs = poll_missing_logs(
                &*historical_indexer.http_provider,
                last_synced as u64,
                max_blocks_per_request,
                &historical_indexer.contract_addrs,
                EVMChain::Ethereum,
            )
            .await?;

            historical_indexer.catchup_missing_blocks(logs).await
        });

        let live_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            info!("Starting live indexing from block {}", current_block + 1);
            let mut stream = subscribe_stream(
                &*live_indexer.ws_provider,
                &live_indexer.contract_addrs,
                EVMChain::Ethereum,
            )
            .await?;

            while let Some(log) = stream.next().await {
                let _ = event_handler.handle_event(log.clone()).await;

                // let _ = self
                //     .db_client
                //     .upsert_last_synced(self.chain_id as i64, log.block_number.unwrap() as i64);
            }
            Ok(())
        });
        let (historical_res, live_res) = tokio::join!(historical_handle, live_handle);
        historical_res??;
        live_res??;

        Ok(())
    }

    fn chain_id(&self) -> u64 {
        self.chain_id
    }
}

impl EthereumIndexer {
    async fn catchup_missing_blocks(&self, logs: Vec<Log>) -> Result<()> {
        let id = self.chain_id();
        for log in logs {

            // match parser::parse_log(log, self.chain_id).await {
            //     Ok(parsed) => {
            //         // let last_synced = last_synced::ActiveModel {
            //         //     chain_id: Set(id as i64),
            //         //     block_number: Set(parsed.block_number),
            //         // };
            //         // db::insert_model(parsed.model, last_synced, &self.db, &self.blockscout_db)
            //         //     .await?;
            //     }
            //     Err(e) => self.handle_error(e)?,
            // }
        }
        Ok(())
    }

    fn handle_error(&self, e: Report) -> Result<()> {
        match e.downcast_ref::<ParserError>() {
            Some(ParserError::UnknownEvent { .. }) => Ok(()),
            _ => {
                error!("Error processing log: {:?}", e);
                Err(e)
            }
        }
    }
}

impl Clone for EthereumIndexer {
    fn clone(&self) -> Self {
        Self {
            ws_provider: Arc::clone(&self.ws_provider),
            http_provider: Arc::clone(&self.http_provider),
            db_client: self.db_client.clone(),
            start_block: self.start_block,
            chain_id: self.chain_id,
            contract_addrs: self.contract_addrs.clone(),
        }
    }
}
