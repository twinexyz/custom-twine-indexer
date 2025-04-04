mod db;
mod parser;

use super::EVMChain;
use crate::entities::last_synced;
use crate::indexer::evm;
use crate::indexer::{ChainIndexer, MAX_RETRIES, RETRY_DELAY};
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Log;
use async_trait::async_trait;
use eyre::{Report, Result};
use futures_util::{future, StreamExt};
use sea_orm::ActiveValue::Set;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{error, info};

pub struct EthereumIndexer {
    /// WS provider for live subscription.
    ws_provider: Arc<dyn Provider + Send + Sync>,
    /// HTTP provider for polling missing blocks.
    http_provider: Arc<dyn Provider + Send + Sync>,
    db: DatabaseConnection,
    blockscout_db: DatabaseConnection,
    chain_id: u64,
    start_block: u64,
    contract_addrs: Vec<String>,
}

#[async_trait]
impl ChainIndexer for EthereumIndexer {
    async fn new(
        http_rpc_url: String,
        ws_rpc_url: String,
        chain_id: u64,
        start_block: u64,
        db: &DatabaseConnection,
        blockscout_db: Option<&DatabaseConnection>,
        contract_addrs: Vec<String>,
    ) -> Result<Self> {
        let ws_provider = super::create_ws_provider(ws_rpc_url, EVMChain::Ethereum).await?;
        let http_provider = super::create_http_provider(http_rpc_url, EVMChain::Ethereum).await?;
        Ok(Self {
            ws_provider: Arc::new(ws_provider),
            http_provider: Arc::new(http_provider),
            db: db.clone(),
            blockscout_db: blockscout_db.unwrap().clone(),
            chain_id,
            start_block,
            contract_addrs,
        })
    }

    async fn run(&mut self) -> Result<()> {
        let id = self.chain_id();

        let last_synced = db::get_last_synced_block(&self.db, id as i64, self.start_block).await?;
        let current_block = self.http_provider.get_block_number().await?;

        let max_blocks_per_request = std::env::var("ETHEREUM__BLOCK_SYNC_BATCH_SIZE")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(100);

        let logs = evm::common::poll_missing_logs(
            &*self.http_provider,
            last_synced as u64,
            max_blocks_per_request,
            &self.contract_addrs,
            EVMChain::Ethereum,
        )
        .await?;
        self.catchup_missing_blocks(logs).await?;

        let live_indexer = self.clone();
        tokio::spawn(async move {
            info!("Starting live indexing from block {}", current_block + 1);
            match evm::common::subscribe_stream(
                &*live_indexer.ws_provider,
                &live_indexer.contract_addrs,
                EVMChain::Ethereum,
            )
            .await
            {
                Ok(mut stream) => {
                    while let Some(log) = stream.next().await {
                        match parser::parse_log(
                            log,
                            &live_indexer.db,
                            &live_indexer.blockscout_db,
                            live_indexer.chain_id,
                        )
                        .await
                        {
                            Ok(parsed) => {
                                let last_synced = last_synced::ActiveModel {
                                    chain_id: Set(id as i64),
                                    block_number: Set(parsed.block_number),
                                };
                                if let Err(e) = db::insert_model(
                                    parsed.model,
                                    last_synced,
                                    &live_indexer.db,
                                    &live_indexer.blockscout_db,
                                )
                                .await
                                {
                                    error!("Failed to insert model: {:?}", e);
                                }
                            }
                            Err(e) => {
                                if let Err(e) = live_indexer.handle_error(e) {
                                    error!("Error processing log: {:?}", e);
                                }
                            }
                        }
                    }
                    info!("Live indexing stream ended");
                }
                Err(e) => error!("Failed to start live stream: {:?}", e),
            }
        });

        future::pending::<()>().await;
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
            match parser::parse_log(log, &self.db, &self.blockscout_db, self.chain_id).await {
                Ok(parsed) => {
                    let last_synced = last_synced::ActiveModel {
                        chain_id: Set(id as i64),
                        block_number: Set(parsed.block_number),
                    };
                    db::insert_model(parsed.model, last_synced, &self.db, &self.blockscout_db)
                        .await?;
                }
                Err(e) => self.handle_error(e)?,
            }
        }
        Ok(())
    }

    fn handle_error(&self, e: Report) -> Result<()> {
        match e.downcast_ref::<parser::ParserError>() {
            Some(parser::ParserError::UnknownEvent { .. }) => Ok(()),
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
            db: self.db.clone(),
            blockscout_db: self.blockscout_db.clone(),
            start_block: self.start_block,
            chain_id: self.chain_id,
            contract_addrs: self.contract_addrs.clone(),
        }
    }
}
