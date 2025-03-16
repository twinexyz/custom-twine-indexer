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
use tokio::task::JoinHandle;

pub struct EVMIndexer {
    provider: Arc<dyn Provider + Send + Sync>,
    db: DatabaseConnection,
}

#[async_trait]
impl ChainIndexer for EVMIndexer {
    async fn new(rpc_url: String, db: &DatabaseConnection) -> Result<Self> {
        let provider = Self::create_provider(rpc_url).await?;
        Ok(Self {
            provider: Arc::new(provider),
            db: db.clone(),
        })
    }

    async fn run(&mut self) -> Result<()> {
        let id = self.chain_id().await?;
        let last_synced = db::get_last_synced_block(&self.db, id as i64).await?;
        let current_block = self.provider.get_block_number().await?;

        // Process historical logs
        let logs = chain::poll_missing_logs(&*self.provider, last_synced as u64).await?;
        self.catchup_missing_blocks(logs).await?;

        // Live indexing in a separate task
        let live_indexer = self.clone();
        tokio::spawn(async move {
            info!("Starting live indexing from block {}", current_block + 1);
            if let Ok(mut stream) = chain::subscribe_stream(&*live_indexer.provider).await {
                while let Some(log) = stream.next().await {
                    match parser::parse_log(log) {
                        Ok(parsed) => {
                            let last_synced = last_synced::ActiveModel {
                                chain_id: Set(id as i64),
                                block_number: Set(parsed.block_number as i64),
                            };
                            if let Err(e) =
                                db::insert_model(parsed.model, last_synced, &live_indexer.db).await
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
            } else {
                error!("Failed to start live stream");
            }
        });

        // Keep the main task alive
        future::pending::<()>().await;
        Ok(())
    }

    async fn chain_id(&self) -> Result<u64> {
        Ok(self.provider.get_chain_id().await?)
    }
}

impl EVMIndexer {
    async fn create_provider(rpc_url: String) -> Result<impl Provider> {
        let ws = WsConnect::new(&rpc_url);
        ProviderBuilder::new().on_ws(ws).await.map_err(Report::from)
    }

    async fn catchup_missing_blocks(&self, logs: Vec<Log>) -> Result<()> {
        let id = self.chain_id().await?;
        for log in logs {
            match parser::parse_log(log) {
                Ok(parsed) => {
                    let last_synced = last_synced::ActiveModel {
                        chain_id: Set(id as i64),
                        block_number: Set(parsed.block_number as i64),
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
                tracing::error!("Error processing log: {:?}", e);
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
        }
    }
}
