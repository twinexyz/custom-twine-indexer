mod db;
mod parser;
mod subscriber;

use std::{str::FromStr, sync::Arc};

use super::{ChainIndexer, MAX_RETRIES, RETRY_DELAY};
use anchor_client::{Client, Cluster};
use async_trait::async_trait;
use eyre::Result;
use futures_util::{future, StreamExt};
use sea_orm::{DatabaseConnection, EntityTrait, Set};
use tracing::{error, info};

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
        let rpc_url = std::env::var("SOLANA__RPC_URL").unwrap_or(http_rpc_url);
        let ws_url = std::env::var("SOLANA_WS_URL").unwrap_or(ws_rpc_url);

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

        // Polling Phase (Catch-Up)
        let historical_events = subscriber::poll_missing_slots(
            &self.rpc_url,
            &self.twine_chain_id,
            &self.tokens_gateway_id,
            last_synced_u64,
            current_slot,
        )
        .await?;

        info!(
            "Found {} historical events from slot {} to {}",
            historical_events.len(),
            last_synced_u64,
            current_slot
        );

        self.catchup_missing_slots(historical_events).await?;

        // Streaming Phase (Live)
        let live_indexer = self.clone();
        tokio::spawn(async move {
            info!("Starting live indexing from slot {}", current_slot + 1);
            match subscriber::subscribe_stream(
                &live_indexer.ws_url,
                &live_indexer.twine_chain_id,
                &live_indexer.tokens_gateway_id,
            )
            .await
            {
                Ok(mut stream) => {
                    while let Some((logs, signature)) = stream.next().await {
                        match parser::parse_log(
                            &logs,
                            signature.clone(),
                            &live_indexer.db,
                            &live_indexer.blockscout_db,
                        )
                        .await
                        {
                            Some(parsed) => {
                                let last_synced = crate::entities::last_synced::ActiveModel {
                                    chain_id: Set(chain_id),
                                    block_number: Set(parsed.slot_number),
                                };
                                if let Err(e) = db::insert_model(
                                    parsed,
                                    last_synced,
                                    &live_indexer.db,
                                    &live_indexer.blockscout_db,
                                )
                                .await
                                {
                                    error!("Failed to insert model: {:?}", e);
                                }
                            }
                            None => {
                                error!("No parsed event from log: {:?}", logs);
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

impl SVMIndexer {
    async fn get_current_slot(&self) -> Result<u64> {
        let rpc_url = self.rpc_url.clone();
        let twine_chain_id = self.twine_chain_id.clone();
        let mut retries = 0;

        loop {
            match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                tokio::task::spawn_blocking({
                    let rpc_url = rpc_url.clone();
                    let twine_chain_id = twine_chain_id.clone();
                    move || {
                        let client: Client<Arc<anchor_client::solana_sdk::signature::Keypair>> =
                            Client::new(
                                Cluster::Custom(rpc_url, "".to_string()),
                                Arc::new(anchor_client::solana_sdk::signature::Keypair::new()),
                            );
                        let program = client.program(
                            anchor_client::solana_sdk::pubkey::Pubkey::from_str(&twine_chain_id)?,
                        )?;
                        program
                            .rpc()
                            .get_slot_with_commitment(
                                anchor_client::solana_sdk::commitment_config::CommitmentConfig::finalized(),
                            )
                            .map_err(|e| eyre::eyre!("Failed to get current slot: {}", e))
                    }
                }),
            )
            .await
            {
                Ok(Ok(slot)) => return Ok(slot?),
                Ok(Err(e)) if retries < MAX_RETRIES => {
                    retries += 1;
                    error!(
                        "Failed to get current slot (attempt {}/{}): {:?}",
                        retries, MAX_RETRIES, e
                    );
                    tokio::time::sleep(RETRY_DELAY).await;
                }
                Ok(Err(e)) => return Err(e.into()),
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    error!(
                        "Timeout fetching current slot (attempt {}/{}): {:?}",
                        retries, MAX_RETRIES, "timeout"
                    );
                    tokio::time::sleep(RETRY_DELAY).await;
                }
                Err(e) => return Err(eyre::eyre!("Timeout after retries: {}", e)),
            }
        }
    }

    async fn catchup_missing_slots(
        &self,
        events: Vec<(Vec<String>, Option<String>)>,
    ) -> Result<()> {
        let chain_id = self.chain_id as i64;
        for (logs, signature) in events {
            match parser::parse_log(&logs, signature.clone(), &self.db, &self.blockscout_db).await {
                Some(parsed) => {
                    let last_synced = crate::entities::last_synced::ActiveModel {
                        chain_id: Set(chain_id),
                        block_number: Set(parsed.slot_number),
                    };
                    db::insert_model(parsed, last_synced, &self.db, &self.blockscout_db).await?;
                }
                None => {
                    error!("No parsed event from historical log: {:?}", logs);
                }
            }
        }
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
