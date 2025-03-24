mod chain;
mod db;
mod parser;

use super::ChainIndexer;
use crate::entities::last_synced;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Log;
use async_trait::async_trait;
use eyre::{Report, Result};
use futures_util::{future, StreamExt};
use sea_orm::ActiveValue::Set;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tracing::{error, info};

pub struct EVMIndexer {
    provider: Arc<dyn Provider + Send + Sync>,
    db: DatabaseConnection,
    chain_id: u64,
    start_block: u64,
    contract_addrs: Vec<String>,
}

#[async_trait]
impl ChainIndexer for EVMIndexer {
    async fn new(
        rpc_url: String,
        chain_id: u64,
        start_block: u64,
        db: &DatabaseConnection,
        contract_addrs: Vec<String>,
    ) -> Result<Self> {
        let provider = Self::create_provider(rpc_url).await?;
        Ok(Self {
            provider: Arc::new(provider),
            db: db.clone(),
            start_block,
            chain_id,
            contract_addrs,
        })
    }

    async fn run(&mut self) -> Result<()> {
        let id = self.chain_id();
        let last_synced = db::get_last_synced_block(&self.db, id as i64, self.start_block).await?;
        let current_block = self.provider.get_block_number().await?;

        info!(
            "Starting EVM indexer for chain {} from block {}",
            id, last_synced
        );

        // Process historical logs
        let logs =
            chain::poll_missing_logs(&*self.provider, last_synced as u64, &self.contract_addrs)
                .await?;
        self.catchup_missing_blocks(logs).await?;

        // Live indexing in a separate task
        let live_indexer = self.clone();
        tokio::spawn(async move {
            info!("Starting live indexing from block {}", current_block + 1);
            match chain::subscribe_stream(&*live_indexer.provider, &live_indexer.contract_addrs)
                .await
            {
                Ok(mut stream) => {
                    while let Some(log) = stream.next().await {
                        match parser::parse_log(log).await {
                            Ok(parsed) => {
                                let last_synced = last_synced::ActiveModel {
                                    chain_id: Set(id as i64),
                                    block_number: Set(parsed.block_number),
                                };
                                if let Err(e) =
                                    db::insert_model(parsed.model, last_synced, &live_indexer.db)
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

        // Keep the main task alive
        future::pending::<()>().await;
        Ok(())
    }

    fn chain_id(&self) -> u64 {
        self.chain_id
    }
}

impl EVMIndexer {
    async fn create_provider(rpc_url: String) -> Result<impl Provider> {
        let ws = WsConnect::new(&rpc_url);
        ProviderBuilder::new().on_ws(ws).await.map_err(Report::from)
    }

    async fn catchup_missing_blocks(&self, logs: Vec<Log>) -> Result<()> {
        let id = self.chain_id();
        for log in logs {
            match parser::parse_log(log).await {
                Ok(parsed) => {
                    let last_synced = last_synced::ActiveModel {
                        chain_id: Set(id as i64),
                        block_number: Set(parsed.block_number),
                    };
                    db::insert_model(parsed.model, last_synced, &self.db).await?;
                }
                Err(e) => self.handle_error(e)?,
            }
        }
        Ok(())
    }

    fn handle_error(&self, e: Report) -> Result<()> {
        match e.downcast_ref::<parser::ParserError>() {
            Some(parser::ParserError::UnknownEvent { .. }) => Ok(()), // Skip unknown events
            _ => {
                error!("Error processing log: {:?}", e);
                Err(e)
            }
        }
    }
}

impl Clone for EVMIndexer {
    fn clone(&self) -> Self {
        Self {
            provider: Arc::clone(&self.provider),
            db: self.db.clone(),
            start_block: self.start_block,
            chain_id: self.chain_id,
            contract_addrs: self.contract_addrs.clone(),
        }
    }
}
