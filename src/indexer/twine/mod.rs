mod chain;
mod db;
mod parser;

use super::{ChainIndexer, MAX_RETRIES, RETRY_DELAY};
use crate::entities::last_synced;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::{Filter, Log};
use async_trait::async_trait;
use eyre::{Report, Result};
use futures_util::StreamExt;
use sea_orm::ActiveValue::Set;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info};

pub struct TwineIndexer {
    /// WS provider for live subscription.
    ws_provider: Arc<dyn Provider + Send + Sync>,
    /// HTTP provider for polling missing blocks.
    http_provider: Arc<dyn Provider + Send + Sync>,
    db: DatabaseConnection,
    chain_id: u64,
    start_block: u64,
    contract_addrs: Vec<String>,
}

#[async_trait]
impl ChainIndexer for TwineIndexer {
    async fn new(
        http_rpc_url: String,
        ws_rpc_url: String,
        chain_id: u64,
        start_block: u64,
        db: &DatabaseConnection,
        contract_addrs: Vec<String>,
    ) -> Result<Self> {
        let ws_provider = Self::create_ws_provider(ws_rpc_url).await?;
        let http_provider = Self::create_http_provider(http_rpc_url).await?;
        Ok(Self {
            ws_provider: Arc::new(ws_provider),
            http_provider: Arc::new(http_provider),
            db: db.clone(),
            chain_id,
            start_block,
            contract_addrs,
        })
    }

    async fn run(&mut self) -> Result<()> {
        let id = self.chain_id();
        let last_synced = db::get_last_synced_block(&self.db, id as i64, self.start_block).await?;
        info!("last synced block is: {last_synced}");
        let current_block = self.http_provider.get_block_number().await?;

        let historical_indexer = self.clone();
        let live_indexer = self.clone();

        let historical_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            info!("Starting historical sync up to block {}", current_block);
            let logs = chain::poll_missing_logs(
                &*historical_indexer.http_provider,
                last_synced as u64,
                &historical_indexer.contract_addrs,
            )
            .await?;

            historical_indexer.catchup_missing_blocks(logs).await
        });

        let live_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            info!("Starting live indexing from block {}", current_block + 1);
            let mut stream =
                chain::subscribe_stream(&*live_indexer.ws_provider, &live_indexer.contract_addrs)
                    .await?;

            while let Some(log) = stream.next().await {
                match parser::parse_log(log) {
                    Ok(parsed) => {
                        let last_synced = last_synced::ActiveModel {
                            chain_id: Set(id as i64),
                            block_number: Set(parsed.block_number),
                        };
                        db::insert_model(parsed.model, last_synced, &live_indexer.db).await;
                    }
                    Err(e) => live_indexer.handle_error(e)?,
                }
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
    async fn create_ws_provider(ws_rpc_url: String) -> Result<impl Provider> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match ProviderBuilder::new()
                .on_ws(WsConnect::new(&ws_rpc_url))
                .await
            {
                Ok(provider) => {
                    info!("Connected to WS provider on attempt {}", attempt);
                    return Ok(provider);
                }
                Err(e) => {
                    error!("WS Attempt {} failed to connect: {}.", attempt, e);
                    if attempt >= MAX_RETRIES {
                        error!("Exceeded maximum WS connection attempts.");
                        return Err(Report::from(e));
                    }
                    // Wait before retrying
                    std::thread::sleep(RETRY_DELAY);
                }
            }
        }
    }

    async fn create_http_provider(http_rpc_url: String) -> Result<impl Provider> {
        let parsed_url = http_rpc_url.parse()?;
        let provider = ProviderBuilder::new().on_http(parsed_url);
        let mut attempt = 0;
        loop {
            attempt += 1;
            // Test if the provider is working by fetching the chain ID
            match provider.get_chain_id().await {
                Ok(chain_id) => {
                    info!(
                        "Successfully connected to HTTP provider on attempt {}.",
                        attempt
                    );
                    return Ok(provider);
                }
                Err(e) => {
                    error!(
                        "Attempt {} failed to connect to HTTP provider: {}",
                        attempt, e
                    );
                    if attempt >= MAX_RETRIES {
                        error!("Exceeded maximum connection attempts.");
                        return Err(eyre::Report::from(e));
                    }
                    tokio::time::sleep(RETRY_DELAY).await;
                }
            }
        }
    }

    async fn catchup_missing_blocks(&self, logs: Vec<Log>) -> Result<()> {
        let id = self.chain_id();
        for log in logs {
            match parser::parse_log(log) {
                Ok(parsed) => {
                    let last_synced = last_synced::ActiveModel {
                        chain_id: Set(id as i64),
                        block_number: Set(parsed.block_number),
                    };
                    db::insert_model(parsed.model, last_synced, &self.db).await;
                }
                Err(e) => self.handle_error(e)?,
            }
        }
        Ok(())
    }

    fn handle_error(&self, e: Report) -> Result<()> {
        match e.downcast_ref::<parser::ParserError>() {
            Some(parser::ParserError::UnknownEvent { .. }) | Some(parser::ParserError::SkipLog) => {
                Ok(())
            } // Skip unknown events
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
            db: self.db.clone(),
            start_block: self.start_block,
            chain_id: self.chain_id,
            contract_addrs: self.contract_addrs.clone(),
        }
    }
}
