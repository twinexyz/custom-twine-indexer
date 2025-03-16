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
    async fn new(rpc_url: String, db: &DatabaseConnection) -> Result<Self> {
        let twine_chain_id = std::env::var("TWINE_CHAIN_PROGRAM_ID")?;
        let tokens_gateway_id = std::env::var("TOKENS_GATEWAY_PROGRAM_ID")?;
        let chain_id = 900;

        // Use SOLANA_RPC_URL if set, otherwise fall back to the provided rpc_url
        let rpc_url = std::env::var("SOLANA_RPC_URL").unwrap_or(rpc_url);

        // Use SOLANA_WS_URL if set, otherwise derive it from rpc_url or use a default
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
        info!("Using RPC URL: {}", self.rpc_url);
        info!("Using WebSocket URL: {}", self.ws_url);

        // Initialization
        let chain_id = self.chain_id as i64;
        let last_synced = db::get_last_synced_slot(&self.db, chain_id).await?;
        let current_slot = self.get_current_slot().await?;
        info!("Last synced slot: {}", last_synced);
        info!("Current slot: {}", current_slot);

        let last_synced_u64 = last_synced.max(0) as u64;

        // Catch-Up Phase
        let historical_events = subscriber::poll_missing_slots(
            &self.rpc_url,
            &self.twine_chain_id,
            &self.tokens_gateway_id,
            last_synced_u64,
        )
        .await?;
        info!(
            "Processing {} historical events from slot {} to {}",
            historical_events.len(),
            last_synced_u64,
            current_slot
        );
        self.catchup_missing_slots(historical_events).await?;

        // Live Phase
        let indexer = self.clone();
        tokio::spawn(async move {
            info!("Starting live indexing from slot {}", current_slot + 1);
            match subscriber::subscribe_stream(
                &indexer.ws_url,
                &indexer.twine_chain_id,
                &indexer.tokens_gateway_id,
            )
            .await
            {
                Ok(mut stream) => {
                    while let Some((encoded_data, signature, event_type)) = stream.next().await {
                        match SVMIndexer::parse_log(&encoded_data, signature.clone(), &event_type) {
                            Ok(parsed) => {
                                let last_synced = crate::entities::last_synced::ActiveModel {
                                    chain_id: Set(indexer.chain_id as i64),
                                    block_number: Set(parsed.slot_number),
                                };
                                // Skip if already processed (by slot or tx_hash)
                                if parsed.slot_number <= current_slot as i64 {
                                    info!(
                                        "Skipping live event at slot {} (already processed)",
                                        parsed.slot_number
                                    );
                                    continue;
                                }

                                match indexer.process_event(parsed.event, &last_synced).await {
                                    Ok(_) => info!(
                                        "Processed live event at slot {} ",
                                        parsed.slot_number
                                    ),
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
                            Err(e) => error!("Failed to parse live log: {:?}", e),
                        }
                    }
                    info!("Live indexing stream ended");
                }
                Err(e) => error!("Failed to subscribe to stream: {:?}", e),
            }
        });

        future::pending::<()>().await;
        Ok(())
    }

    async fn chain_id(&self) -> Result<u64> {
        Ok(self.chain_id)
    }
}

impl SVMIndexer {
    async fn get_current_slot(&self) -> Result<u64> {
        let rpc_url = self.rpc_url.clone();
        let twine_chain_id = self.twine_chain_id.clone();
        tokio::task::spawn_blocking(move || {
            let client: Client<Arc<anchor_client::solana_sdk::signature::Keypair>> = Client::new(
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
        .await?
    }

    fn parse_log(
        encoded_data: &str,
        signature: Option<String>,
        event_type: &str,
    ) -> Result<parser::ParsedEvent> {
        parser::parse_log(encoded_data, signature, event_type)
            .map_err(|e| eyre::eyre!("Failed to parse log: {}", e))
    }

    async fn is_event_processed(&self, tx_hash: &str) -> Result<bool> {
        use crate::entities::l1_deposit;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

        let exists = l1_deposit::Entity::find()
            .filter(l1_deposit::Column::TxHash.eq(tx_hash))
            .one(&self.db)
            .await?;
        Ok(exists.is_some())
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
            serde_json::from_value::<parser::NativeWithdrawalSuccessful>(event_clone.clone())
        {
            info!(
                "Inserting native withdrawal successful with nonce: {}, slot: {}, tx_hash: {}",
                native_success.nonce, native_success.slot_number, native_success.signature
            );
            db::insert_native_withdrawal_successful(&native_success, db).await?;
        } else if let Ok(spl_success) =
            serde_json::from_value::<parser::SplWithdrawalSuccessful>(event_clone)
        {
            info!(
                "Inserting SPL withdrawal successful with nonce: {}, slot: {}, tx_hash: {}",
                spl_success.nonce, spl_success.slot_number, spl_success.signature
            );
            db::insert_spl_withdrawal_successful(&spl_success, db).await?;
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

    async fn catchup_missing_slots(
        &self,
        events: Vec<(String, Option<String>, String)>,
    ) -> Result<()> {
        for (encoded_data, signature, event_type) in events {
            match Self::parse_log(&encoded_data, signature.clone(), &event_type) {
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
            ws_url: self.ws_url.clone(),
            twine_chain_id: self.twine_chain_id.clone(),
            tokens_gateway_id: self.tokens_gateway_id.clone(),
            chain_id: self.chain_id,
        }
    }
}
