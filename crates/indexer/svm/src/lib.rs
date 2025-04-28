mod db;
mod parser;
mod subscriber;

use std::{str::FromStr, sync::Arc};

use alloy::transports::http::reqwest;
use anchor_client::{Client, Cluster};
use async_trait::async_trait;
use common::{
    entities::last_synced,
    indexer::{ChainIndexer, MAX_RETRIES, RETRY_DELAY},
};
use eyre::Result;
use futures_util::{future, Stream, StreamExt};
use sea_orm::{DatabaseConnection, EntityTrait, Set};
use serde_json::{json, Value};
use tokio::task::JoinHandle;
use tracing::{debug, error, info};

pub struct SVMIndexer {
    db: DatabaseConnection,
    blockscout_db: DatabaseConnection,
    rpc_url: String,
    ws_url: String,
    twine_chain_id: String,
    tokens_gateway_id: String,
    chain_id: u64,
    start_block: u64,
}

#[async_trait]
impl ChainIndexer for SVMIndexer {
    async fn new(
        http_rpc_url: String,
        ws_rpc_url: String,
        chain_id: u64,
        start_block: u64,
        db: &DatabaseConnection,
        blockscout_db: Option<&DatabaseConnection>,
        _contract_addrs: Vec<String>,
    ) -> Result<Self> {
        let twine_chain_id = std::env::var("TWINE_CHAIN_PROGRAM_ADDRESS")?;
        let tokens_gateway_id = std::env::var("TOKENS_GATEWAY_PROGRAM_ADDRESS")?;
        let rpc_url = std::env::var("SOLANA__HTTP_RPC_URL").unwrap_or(http_rpc_url);
        let ws_url = std::env::var("SOLANA__WS_RPC_URL").unwrap_or(ws_rpc_url);

        Ok(Self {
            db: db.clone(),
            blockscout_db: blockscout_db.unwrap().clone(),
            rpc_url,
            ws_url,
            twine_chain_id,
            tokens_gateway_id,
            chain_id,
            start_block,
        })
    }

    async fn run(&mut self) -> Result<()> {
        let chain_id = self.chain_id as i64;
        let last_synced = db::get_last_synced_slot(&self.db, chain_id, self.start_block).await?;
        let current_slot = self.get_current_slot().await?;
        let last_synced_u64 = last_synced.max(0) as u64;

        info!("Last synced slot: {}", last_synced);
        info!("Current slot: {}", current_slot);

        let max_slots_per_request = std::env::var("SOLANA__BLOCK_SYNC_BATCH_SIZE")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(100); // Match log batch size

        let historical_indexer = self.clone();
        let live_indexer = self.clone();

        let historical_handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            info!(
                "Starting historical sync from slot {} to {}",
                last_synced_u64 + 1,
                current_slot
            );
            let commit_batch_stream = subscriber::poll_missing_slots(
                &historical_indexer.rpc_url,
                &historical_indexer.twine_chain_id,
                &historical_indexer.tokens_gateway_id,
                last_synced_u64,
                current_slot,
                max_slots_per_request,
            );

            info!("Processing events from historical stream");
            match historical_indexer
                .catchup_missing_slots(commit_batch_stream)
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
                        debug!("Processing live event with signature {:?}", signature);
                        match parser::parse_log(
                            &logs,
                            signature.clone(),
                            &live_indexer.db,
                            &live_indexer.blockscout_db,
                        )
                        .await
                        {
                            Some(parsed_event) => {
                                let last_synced = last_synced::ActiveModel {
                                    chain_id: Set(live_indexer.chain_id as i64),
                                    block_number: Set(parsed_event.slot_number),
                                    ..Default::default()
                                };
                                if let Err(e) = db::insert_model(
                                    parsed_event,
                                    last_synced,
                                    &live_indexer.db,
                                    &live_indexer.blockscout_db,
                                )
                                .await
                                {
                                    error!(
                                        "Failed to insert live event with signature {:?}: {:?}",
                                        signature, e
                                    );
                                } else {
                                    info!("Inserted live event with signature {:?}", signature);
                                }
                            }
                            None => {
                                debug!(
                                    "No recognized event parsed for signature {:?}: logs={:?}",
                                    signature, logs
                                );
                            }
                        }
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
        mut events: impl Stream<Item = Result<(Vec<String>, Option<String>)>> + Unpin,
    ) -> Result<()> {
        let mut processed_count = 0;
        while let Some(event) = events.next().await {
            let (logs, signature) = event?;
            debug!("Processing historical event with signature {:?}", signature);
            match parser::parse_log(&logs, signature.clone(), &self.db, &self.blockscout_db).await {
                Some(parsed_event) => {
                    let last_synced = last_synced::ActiveModel {
                        chain_id: Set(self.chain_id as i64),
                        block_number: Set(parsed_event.slot_number),
                        ..Default::default()
                    };
                    if let Err(e) =
                        db::insert_model(parsed_event, last_synced, &self.db, &self.blockscout_db)
                            .await
                    {
                        error!(
                            "Failed to insert historical event with signature {:?}: {:?}",
                            signature, e
                        );
                    } else {
                        info!("Inserted historical event with signature {:?}", signature);
                    }
                }
                None => {
                    debug!(
                        "No recognized event parsed for signature {:?}: logs={:?}",
                        signature, logs
                    );
                }
            }
            processed_count += 1;
            info!("Processed {} events so far", processed_count);
        }
        info!("Completed processing {} events", processed_count);
        Ok(())
    }
}

impl Clone for SVMIndexer {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            blockscout_db: self.blockscout_db.clone(),
            rpc_url: self.rpc_url.clone(),
            ws_url: self.ws_url.clone(),
            twine_chain_id: self.twine_chain_id.clone(),
            tokens_gateway_id: self.tokens_gateway_id.clone(),
            chain_id: self.chain_id,
            start_block: self.start_block,
        }
    }
}
