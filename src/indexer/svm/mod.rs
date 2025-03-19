// mod.rs
mod db;
mod parser;
mod subscriber;

use std::{str::FromStr, sync::Arc};

use super::ChainIndexer;
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
}

#[async_trait]
impl ChainIndexer for SVMIndexer {
    async fn new(rpc_url: String, chain_id: u64, db: &DatabaseConnection) -> Result<Self> {
        let twine_chain_id = std::env::var("TWINE_CHAIN_PROGRAM_ID")?;
        let tokens_gateway_id = std::env::var("TOKENS_GATEWAY_PROGRAM_ID")?;

        let rpc_url = std::env::var("SOLANA_RPC_URL").unwrap_or(rpc_url);
        let ws_url = std::env::var("SOLANA_WS_URL").unwrap_or_else(|_| {
            if rpc_url.starts_with("http://") {
                rpc_url
                    .replace("http://", "ws://")
                    .replace(":8899", ":8900")
            } else if rpc_url.starts_with("https://") {
                rpc_url.replace("https://", "wss://")
            } else if rpc_url.starts_with("ws://") || rpc_url.starts_with("wss://") {
                rpc_url.clone()
            } else {
                format!("wss://{}", rpc_url)
            }
        });

        Ok(Self {
            db: db.clone(),
            rpc_url,
            ws_url,
            twine_chain_id,
            tokens_gateway_id,
            chain_id,
        })
    }

    async fn run(&mut self) -> Result<()> {
        let chain_id = self.chain_id as i64;

        let last_synced = db::get_last_synced_slot(&self.db, chain_id).await?;
        let current_slot = self.get_current_slot().await?;

        info!("Last synced slot: {}", last_synced);
        info!("Current slot: {}", current_slot);

        let last_synced_u64 = last_synced.max(0) as u64;

        // Catch-Up Phase
        if last_synced_u64 < current_slot {
            let historical_events = subscriber::poll_missing_slots(
                &self.rpc_url,
                &self.twine_chain_id,
                &self.tokens_gateway_id,
                last_synced_u64,
            )
            .await?;

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
                        info!("Skipping already processed tx: {}", tx_hash);
                        continue;
                    }
                }

                match parser::parse_log(&logs, signature.clone()) {
                    Ok(parsed) => {
                        if parsed.slot_number <= last_synced {
                            info!(
                                "Skipping historical event at slot {} (before last synced)",
                                parsed.slot_number
                            );
                            continue;
                        }
                        if parsed.slot_number > current_slot as i64 {
                            info!(
                                "Skipping historical event at slot {} (after current slot)",
                                parsed.slot_number
                            );
                            continue;
                        }

                        let last_synced_model = crate::entities::last_synced::ActiveModel {
                            chain_id: Set(self.chain_id as i64),
                            block_number: Set(parsed.slot_number),
                        };
                        Self::process_event_static(&self.db, parsed.event, &last_synced_model)
                            .await?;
                        processed_count += 1;
                    }
                    Err(e) => error!("Failed to parse historical logs: {:?}", e),
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
                let mut stream = subscriber::subscribe_stream(
                    &indexer.ws_url,
                    &indexer.twine_chain_id,
                    &indexer.tokens_gateway_id,
                )
                .await?;

                while let Some((logs, signature)) = stream.next().await {
                    match parser::parse_log(&logs, signature.clone()) {
                        Ok(parsed) => {
                            if parsed.slot_number <= current_slot as i64 {
                                info!(
                                    "Skipping live event at slot {} (already processed)",
                                    parsed.slot_number
                                );
                                continue;
                            }

                            if let Some(tx_hash) = &signature {
                                if db::is_tx_hash_processed(&indexer.db, tx_hash).await? {
                                    info!("Skipping already processed live tx: {}", tx_hash);
                                    continue;
                                }
                            }

                            let last_synced = crate::entities::last_synced::ActiveModel {
                                chain_id: Set(indexer.chain_id as i64),
                                block_number: Set(parsed.slot_number),
                            };

                            match indexer.process_event(parsed.event, &last_synced).await {
                                Ok(_) => {
                                    info!("Processed live event at slot {} ", parsed.slot_number)
                                }
                                Err(e) => {
                                    if e.to_string().contains("duplicate key value") {
                                        info!(
                                            "Skipping duplicate live event at slot {}",
                                            parsed.slot_number
                                        );
                                    } else {
                                        error!("Failed to process live event: {:?}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => error!("Failed to parse live logs: {:?}", e),
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

    fn chain_id(&self) -> u64{
        self.chain_id
    }
}

impl SVMIndexer {
    async fn get_current_slot(&self) -> Result<u64> {
        let rpc_url = self.rpc_url.clone();
        let twine_chain_id = self.twine_chain_id.clone();
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::task::spawn_blocking(move || {
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
            }),
        )
        .await
        .map_err(|_| eyre::eyre!("Timeout while fetching current slot"))?
        .map_err(|e| eyre::eyre!("Failed to fetch current slot: {}", e))?
    }

    async fn process_event_static(
        db: &DatabaseConnection,
        event: serde_json::Value,
        last_synced: &crate::entities::last_synced::ActiveModel,
    ) -> Result<()> {
        let event_clone = event.clone();

        if let Ok(deposit) =
            serde_json::from_value::<parser::DepositSuccessful>(event_clone.clone())
        {
            let is_native = deposit.l1_token == "11111111111111111111111111111111";
            info!(
                "Inserting {} deposit with nonce: {}, slot: {}, tx_hash: {}",
                if is_native { "native" } else { "SPL" },
                deposit.nonce,
                deposit.slot_number,
                deposit.signature
            );
            if is_native {
                db::insert_sol_deposit(&deposit, db).await?;
            } else {
                db::insert_spl_deposit(&deposit, db).await?;
            }
        } else if let Ok(withdrawal) =
            serde_json::from_value::<parser::ForcedWithdrawSuccessful>(event_clone.clone())
        {
            let is_native = withdrawal.l1_token == "11111111111111111111111111111111";
            info!(
                "Inserting {} withdrawal with nonce: {}, slot: {}, tx_hash: {}",
                if is_native { "native" } else { "SPL" },
                withdrawal.nonce,
                withdrawal.slot_number,
                withdrawal.signature
            );
            if is_native {
                db::insert_native_withdrawal(&withdrawal, db).await?;
            } else {
                db::insert_spl_withdrawal(&withdrawal, db).await?;
            }
        } else if let Ok(native_success) =
            serde_json::from_value::<parser::FinalizeNativeWithdrawal>(event_clone.clone())
        {
            info!(
                "Inserting native withdrawal successful with nonce: {}, slot: {}, tx_hash: {}",
                native_success.nonce, native_success.slot_number, native_success.signature
            );
            db::insert_finalize_native_withdrawal(&native_success, db).await?;
        } else if let Ok(spl_success) =
            serde_json::from_value::<parser::FinalizeSplWithdrawal>(event_clone)
        {
            info!(
                "Inserting SPL withdrawal successful with nonce: {}, slot: {}, tx_hash: {}",
                spl_success.nonce, spl_success.slot_number, spl_success.signature
            );
            db::insert_finalize_spl_withdrawal(&spl_success, db).await?;
        } else {
            error!("Failed to deserialize event: {:?}", event);
            return Err(eyre::eyre!("Unknown event type"));
        }

        let chain_id = last_synced.chain_id.as_ref();
        let block_number = last_synced.block_number.as_ref();
        crate::entities::last_synced::Entity::insert(last_synced.clone())
            .on_conflict(
                sea_query::OnConflict::column(crate::entities::last_synced::Column::ChainId)
                    .update_column(crate::entities::last_synced::Column::BlockNumber)
                    .to_owned(),
            )
            .exec(db)
            .await?;
        info!(
            "Updated last_synced for chain_id: {}, slot: {}",
            chain_id, block_number
        );
        Ok(())
    }

    async fn process_event(
        &self,
        event: serde_json::Value,
        last_synced: &crate::entities::last_synced::ActiveModel,
    ) -> Result<()> {
        Self::process_event_static(&self.db, event, last_synced).await
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
        }
    }
}
