mod handler;
mod parser;
mod subscriber;
mod provider;
mod indexer;

use std::{str::FromStr, sync::Arc};

use alloy::transports::http::reqwest;
use anchor_client::{Client, Cluster};
use async_trait::async_trait;
use common::config::SvmConfig;
use common::indexer::{MAX_RETRIES, RETRY_DELAY};
use database::client::DbClient;
use eyre::Result;
use futures_util::{future, StreamExt};
use handler::SolanaEventHandler;
use sea_orm::{DatabaseConnection, EntityTrait, Set};
use serde_json::{json, Value};
use tokio::task::JoinHandle;
use tracing::{debug, error, info};


pub struct SVMIndexer {
    rpc_url: String,
    ws_url: String,
    twine_chain_id: String,
    tokens_gateway_id: String,
    chain_id: u64,
    start_block: u64,
    db_client: Arc<DbClient>,
}

// #[async_trait]
impl SVMIndexer {
    pub async fn new(config: SvmConfig, db_client: Arc<DbClient>) -> Result<Self> {
        let twine_chain_id = config.twine_chain_program_address;
        let tokens_gateway_id = config.tokens_gateway_program_address;
        let rpc_url = config.common.http_rpc_url;
        let ws_url = config.common.ws_rpc_url;
        let chain_id = config.common.chain_id;

        Ok(Self {
            rpc_url,
            ws_url,
            twine_chain_id,
            tokens_gateway_id,
            chain_id,
            start_block: config.common.start_block,
            db_client,
        })
    }

    async fn run(&mut self) -> Result<()> {
        let chain_id = self.chain_id as i64;
        let current_slot = self.get_current_slot().await?;
        let last_synced = self
            .db_client
            .get_last_synced_height(chain_id, current_slot)
            .await?;
        let last_synced_u64 = last_synced.max(0) as u64;

        info!("Last synced slot: {}", last_synced);
        info!("Current slot: {}", current_slot);

        let max_slots_per_request = std::env::var("SOLANA__BLOCK_SYNC_BATCH_SIZE")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1000);

        let historical_indexer = self.clone();
        let live_indexer = self.clone();

        let event_handlers = SolanaEventHandler::new(self.db_client.clone(), self.chain_id);

        let historical_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            info!(
                "Starting historical sync from slot {} to {}",
                last_synced_u64 + 1,
                current_slot
            );
            let historical_events = match subscriber::poll_missing_slots(
                &historical_indexer.rpc_url,
                &historical_indexer.twine_chain_id,
                &historical_indexer.tokens_gateway_id,
                last_synced_u64,
                current_slot,
                max_slots_per_request,
            )
            .await
            {
                Ok(events) => {
                    info!("Historical sync returned {} events", events.len());
                    events
                }
                Err(e) => {
                    error!("Historical sync failed: {:?}", e);
                    return Err(e);
                }
            };

            info!("Processing {} historical events", historical_events.len());
            match historical_indexer
                .catchup_missing_slots(historical_events)
                .await
            {
                Ok(()) => info!("Historical sync completed successfully"),
                Err(e) => {
                    error!("Failed to process historical events: {:?}", e);
                    return Err(e);
                }
            }
            Ok(())
        });

        // Live indexing starts from current_slot + 1
        let live_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            info!("Starting live indexing from slot {}", current_slot);
            match subscriber::subscribe_stream(
                &live_indexer.ws_url,
                &live_indexer.twine_chain_id,
                &live_indexer.tokens_gateway_id,
            )
            .await
            {
                Ok(mut stream) => {
                    while let Some((logs, signature)) = stream.next().await {
                        let _ = event_handlers.handle_event(&logs, signature.clone()).await;
                    }
                    info!("Live indexing stream ended");
                }
                Err(e) => error!("Failed to start live stream: {:?}", e),
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

impl SVMIndexer {
    async fn get_current_slot(&self) -> Result<u64> {
        let rpc_url = self.rpc_url.clone();
        let mut retries = 0;

        loop {
            match self.fetch_slot(&rpc_url).await {
                Ok(slot) => return Ok(slot),
                Err(e) if retries < MAX_RETRIES => {
                    retries += 1;
                    error!(
                        "Failed to get current slot (attempt {}/{}): {:?}",
                        retries, MAX_RETRIES, e
                    );
                    tokio::time::sleep(RETRY_DELAY).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn fetch_slot(&self, rpc_url: &str) -> Result<u64> {
        let client = reqwest::Client::new();
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getSlot",
            "params": [
                {"commitment": "finalized"}
            ]
        });

        let response = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            client.post(rpc_url).json(&request_body).send(),
        )
        .await??;

        let json: Value = response.json().await?;
        let slot = json["result"]
            .as_u64()
            .ok_or_else(|| eyre::eyre!("Failed to parse slot from response: {:?}", json))?;

        Ok(slot)
    }

    async fn catchup_missing_slots(
        &self,
        events: Vec<(Vec<String>, Option<String>)>,
    ) -> Result<()> {
        let chain_id = self.chain_id as i64;
        let mut parsed_count = 0;
        let event_handlers = SolanaEventHandler::new(self.db_client.clone(), self.chain_id);

        for (logs, signature) in events {
            let _ = event_handlers.handle_event(&logs, signature.clone()).await;
        }
        info!("Parsed and inserted {} historical events", parsed_count);
        Ok(())
    }
}

impl Clone for SVMIndexer {
    fn clone(&self) -> Self {
        Self {
            db_client: self.db_client.clone(),
            rpc_url: self.rpc_url.clone(),
            ws_url: self.ws_url.clone(),
            twine_chain_id: self.twine_chain_id.clone(),
            tokens_gateway_id: self.tokens_gateway_id.clone(),
            chain_id: self.chain_id,
            start_block: self.start_block,
        }
    }
}
