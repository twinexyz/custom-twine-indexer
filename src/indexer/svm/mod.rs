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
        _contract_addrs: Vec<String>,
    ) -> Result<Self> {
        let twine_chain_id = std::env::var("TWINE_CHAIN_PROGRAM_ADDRESS")?;
        let tokens_gateway_id = std::env::var("TOKENS_GATEWAY_PROGRAM_ADDRESS")?;
        let rpc_url = std::env::var("SOLANA__RPC_URL").unwrap_or(http_rpc_url);
        let ws_url = std::env::var("SOLANA_WS_URL").unwrap_or(ws_rpc_url);
        println!("twine chain id: {}", twine_chain_id);
        println!("tokens gateway id: {}", tokens_gateway_id);

        Ok(Self {
            db: db.clone(),
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

        info!("Last synced slot: {}", last_synced);
        info!("Current slot: {}", current_slot);

        let last_synced_u64 = last_synced.max(0) as u64;

        // Catch-Up Phase
        if last_synced_u64 < current_slot {
            let mut retries = 0;
            let historical_events = loop {
                match subscriber::poll_missing_slots(
                    &self.rpc_url,
                    &self.twine_chain_id,
                    &self.tokens_gateway_id,
                    last_synced_u64,
                )
                .await
                {
                    Ok(events) => break events,
                    Err(e) if retries < MAX_RETRIES => {
                        retries += 1;
                        error!(
                            "Failed to poll missing slots (attempt {}/{}): {:?}",
                            retries, MAX_RETRIES, e
                        );
                        tokio::time::sleep(RETRY_DELAY).await;
                    }
                    Err(e) => return Err(e),
                }
            };

            info!(
                "Found {} historical events from slot {} to {}",
                historical_events.len(),
                last_synced_u64,
                current_slot
            );

            let mut processed_count = 0;
            for (logs, signature) in historical_events {
                if let Some(tx_hash) = &signature {
                    if db::is_tx_hash_processed(&self.db, tx_hash).await? {
                        continue;
                    }
                }

                if let Some(parsed) = parser::parse_log(&logs, signature.clone(), &self.db).await {
                    if parsed.slot_number <= last_synced {
                        continue;
                    }
                    if parsed.slot_number > current_slot as i64 {
                        continue;
                    }

                    let last_synced_model = crate::entities::last_synced::ActiveModel {
                        chain_id: Set(self.chain_id as i64),
                        block_number: Set(parsed.slot_number),
                    };
                    Self::process_event_static(&self.db, parsed.model, &last_synced_model).await?;
                    processed_count += 1;
                }
            }

            info!(
                "Processed {} new historical events from slot {} to {}",
                processed_count, last_synced_u64, current_slot
            );
        } else {
            info!("No historical events to process");
        }

        // Live Phase
        let indexer = self.clone();
        tokio::spawn(async move {
            info!("Starting live indexing from slot {}", current_slot + 1);
            if let Err(e) = async {
                let mut retries = 0;
                let mut stream = loop {
                    match subscriber::subscribe_stream(
                        &indexer.ws_url,
                        &indexer.twine_chain_id,
                        &indexer.tokens_gateway_id,
                    )
                    .await
                    {
                        Ok(stream) => break stream,
                        Err(e) if retries < MAX_RETRIES => {
                            retries += 1;
                            error!(
                                "Failed to subscribe to stream (attempt {}/{}): {:?}",
                                retries, MAX_RETRIES, e
                            );
                            tokio::time::sleep(RETRY_DELAY).await;
                        }
                        Err(e) => return Err(e),
                    }
                };

                while let Some((logs, signature)) = stream.next().await {
                    if let Some(parsed) =
                        parser::parse_log(&logs, signature.clone(), &indexer.db).await
                    {
                        if parsed.slot_number <= current_slot as i64 {
                            continue;
                        }

                        if let Some(tx_hash) = &signature {
                            if db::is_tx_hash_processed(&indexer.db, tx_hash).await? {
                                continue;
                            }
                        }

                        let last_synced = crate::entities::last_synced::ActiveModel {
                            chain_id: Set(indexer.chain_id as i64),
                            block_number: Set(parsed.slot_number),
                        };

                        match indexer.process_event(parsed.model, &last_synced).await {
                            Ok(_) => info!("Processed live event at slot {}", parsed.slot_number),
                            Err(e) => {
                                if e.to_string().contains("duplicate key value") {
                                    // Ignore duplicate key errors
                                } else {
                                    error!("Failed to process live event: {:?}", e);
                                }
                            }
                        }
                    }
                }
                Ok::<(), eyre::Error>(())
            }
            .await
            {
                error!("Live indexing failed: {:?}", e);
            }
            info!("Live indexing stream ended");
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

    async fn process_event_static(
        db: &DatabaseConnection,
        model: parser::DbModel,
        last_synced: &crate::entities::last_synced::ActiveModel,
    ) -> Result<()> {
        db::insert_model(
            parser::ParsedEvent {
                model,
                slot_number: last_synced.block_number.as_ref().clone(),
            },
            last_synced.clone(),
            db,
        )
        .await?;
        info!(
            "Processed event and updated last_synced for chain_id: {}, slot: {}",
            last_synced.chain_id.as_ref(),
            last_synced.block_number.as_ref()
        );
        Ok(())
    }

    async fn process_event(
        &self,
        model: parser::DbModel,
        last_synced: &crate::entities::last_synced::ActiveModel,
    ) -> Result<()> {
        Self::process_event_static(&self.db, model, last_synced).await
    }
}

impl Clone for SVMIndexer {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            rpc_url: self.rpc_url.clone(),
            ws_url: self.ws_url.clone(),
            twine_chain_id: self.twine_chain_id.clone(),
            tokens_gateway_id: self.tokens_gateway_id.clone(),
            chain_id: self.chain_id,
            start_block: self.start_block,
        }
    }
}
