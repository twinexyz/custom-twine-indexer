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
    twine_chain_id: String,
    tokens_gateway_id: String,
    chain_id: u64,
    // Removed `client` field since it can't be cloned
}

#[async_trait]
impl ChainIndexer for SVMIndexer {
    async fn new(rpc_url: String, db: &DatabaseConnection) -> Result<Self> {
        let twine_chain_id = std::env::var("TWINE_CHAIN_PROGRAM_ID")?;
        let tokens_gateway_id = std::env::var("TOKENS_GATEWAY_PROGRAM_ID")?;
        let chain_id = 900; // Hardcoded as before

        Ok(Self {
            db: db.clone(),
            rpc_url,
            twine_chain_id,
            tokens_gateway_id,
            chain_id,
        })
    }

    async fn run(&mut self) -> Result<()> {
        let ws_url = if self.rpc_url.starts_with("http://") {
            self.rpc_url.replace("http://", "ws://")
        } else if self.rpc_url.starts_with("https://") {
            self.rpc_url.replace("https://", "wss://")
        } else if self.rpc_url.starts_with("ws://") || self.rpc_url.starts_with("wss://") {
            self.rpc_url.clone()
        } else {
            format!("wss://{}", self.rpc_url)
        };

        let chain_id = self.chain_id as i64;
        info!("Fetching last synced slot for chain_id: {}", chain_id);
        let last_synced = db::get_last_synced_slot(&self.db, chain_id).await?;
        info!("Last synced slot: {}", last_synced);

        // Get current slot using spawn_blocking for synchronous RPC call
        let rpc_url = self.rpc_url.clone(); // Clone for closure
        let twine_chain_id = self.twine_chain_id.clone(); // Clone for closure
        let current_slot = tokio::task::spawn_blocking(move || {
            let client = Client::new(
                Cluster::Custom(rpc_url, "".to_string()),
                Arc::new(anchor_client::solana_sdk::signature::Keypair::new()),
            );
            let program = client.program(anchor_client::solana_sdk::pubkey::Pubkey::from_str(
                &twine_chain_id,
            )?)?;
            program
                .rpc()
                .get_slot_with_commitment(
                    anchor_client::solana_sdk::commitment_config::CommitmentConfig::finalized(),
                )
                .map_err(|e| eyre::eyre!("Failed to get current slot: {}", e))
        })
        .await??;

        info!("Current slot is: {}", current_slot);
        info!("Last synced slot is: {}", last_synced);

        // Process historical events
        let last_synced_u64 = if last_synced >= 0 {
            last_synced
                .try_into()
                .map_err(|_| eyre::eyre!("Failed to convert last_synced to u64: {}", last_synced))?
        } else {
            return Err(eyre::eyre!(
                "last_synced cannot be negative: {}",
                last_synced
            ));
        };

        let historical_events = if last_synced_u64 < current_slot {
            tokio::time::timeout(
                std::time::Duration::from_secs(30),
                subscriber::poll_missing_slots(
                    &self.rpc_url,
                    &self.twine_chain_id,
                    &self.tokens_gateway_id,
                    last_synced_u64,
                ),
            )
            .await
            .map_err(|_| eyre::eyre!("Historical polling timed out after 30 seconds"))?
            .map_err(|e| eyre::eyre!("Failed to poll missing slots: {:?}", e))?
        } else {
            Vec::new()
        };

        info!("Processing {} historical events", historical_events.len());
        self.catchup_missing_slots(historical_events).await?;

        // Live indexing in a separate task
        let indexer = self.clone();
        tokio::spawn(async move {
            info!("Starting live indexing from slot {}", current_slot + 1);
            match subscriber::subscribe_stream(
                &ws_url,
                &indexer.twine_chain_id,
                &indexer.tokens_gateway_id,
            )
            .await
            {
                Ok(mut stream) => {
                    while let Some((encoded_data, signature, event_type)) = stream.next().await {
                        match SVMIndexer::parse_log(&encoded_data, signature, &event_type) {
                            Ok(parsed) => {
                                let last_synced = crate::entities::last_synced::ActiveModel {
                                    chain_id: Set(indexer.chain_id as i64),
                                    block_number: Set(parsed.slot_number),
                                };
                                if let Err(e) =
                                    indexer.process_event(parsed.event, &last_synced).await
                                {
                                    error!("Failed to process event: {:?}", e);
                                }
                            }
                            Err(e) => error!("Failed to parse log: {:?}", e),
                        }
                    }
                    info!("Live indexing stream ended");
                }
                Err(e) => error!("Failed to subscribe to stream: {:?}", e),
            }
        });

        // Keep the main task alive
        future::pending::<()>().await;
        Ok(())
    }

    async fn chain_id(&self) -> Result<u64> {
        Ok(self.chain_id)
    }
}

impl SVMIndexer {
    fn parse_log(
        encoded_data: &str,
        signature: Option<String>,
        event_type: &str,
    ) -> Result<parser::ParsedEvent> {
        parser::parse_log(encoded_data, signature, event_type)
            .map_err(|e| eyre::eyre!("Failed to parse log: {}", e))
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
            if is_native {
                db::insert_sol_deposit(&deposit, db).await?;
                info!("✅ Inserted native deposit with nonce: {}", deposit.nonce);
            } else {
                db::insert_spl_deposit(&deposit, db).await?;
                info!("✅ Inserted SPL deposit with nonce: {}", deposit.nonce);
            }
        } else if let Ok(withdrawal) =
            serde_json::from_value::<parser::ForcedWithdrawSuccessful>(event_clone.clone())
        {
            let is_native = withdrawal.l1_token == "11111111111111111111111111111111";
            if is_native {
                db::insert_native_withdrawal(&withdrawal, db).await?;
                info!(
                    "✅ Inserted native withdrawal with nonce: {}",
                    withdrawal.nonce
                );
            } else {
                db::insert_spl_withdrawal(&withdrawal, db).await?;
                info!(
                    "✅ Inserted SPL withdrawal with nonce: {}",
                    withdrawal.nonce
                );
            }
        } else if let Ok(native_success) =
            serde_json::from_value::<parser::NativeWithdrawalSuccessful>(event_clone.clone())
        {
            db::insert_native_withdrawal_successful(&native_success, db).await?;
            info!(
                "✅ Inserted native withdrawal successful with nonce: {}",
                native_success.nonce
            );
        } else if let Ok(spl_success) =
            serde_json::from_value::<parser::SplWithdrawalSuccessful>(event_clone)
        {
            db::insert_spl_withdrawal_successful(&spl_success, db).await?;
            info!(
                "✅ Inserted SPL withdrawal successful with nonce: {}",
                spl_success.nonce
            );
        } else {
            error!("Failed to deserialize event: {:?}", event);
            return Err(eyre::eyre!("Unknown event type"));
        }

        if let Err(e) = crate::entities::last_synced::Entity::insert(last_synced.clone())
            .on_conflict(
                sea_query::OnConflict::column(crate::entities::last_synced::Column::ChainId)
                    .update_column(crate::entities::last_synced::Column::BlockNumber)
                    .to_owned(),
            )
            .exec(db)
            .await
        {
            error!("Failed to upsert last synced slot: {:?}", e);
        }

        Ok(())
    }

    async fn catchup_missing_slots(
        &self,
        events: Vec<(String, Option<String>, String)>,
    ) -> Result<()> {
        for (encoded_data, signature, event_type) in events {
            match Self::parse_log(&encoded_data, signature, &event_type) {
                Ok(parsed) => {
                    let last_synced = crate::entities::last_synced::ActiveModel {
                        chain_id: Set(self.chain_id as i64),
                        block_number: Set(parsed.slot_number),
                    };
                    Self::process_event_static(&self.db, parsed.event, &last_synced).await?;
                }
                Err(e) => error!("Failed to parse historical log: {:?}", e),
            }
        }
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
            twine_chain_id: self.twine_chain_id.clone(),
            tokens_gateway_id: self.tokens_gateway_id.clone(),
            chain_id: self.chain_id,
        }
    }
}
