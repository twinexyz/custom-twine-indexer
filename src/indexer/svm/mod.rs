mod db;
mod parser;
mod subscriber;
mod utils;

use super::ChainIndexer;
use async_trait::async_trait;
use eyre::Result;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use tokio::sync::mpsc::{self, Receiver};
use tracing::{error, info};

pub struct SVMIndexer {
    db: DatabaseConnection,
    event_receiver: Receiver<Value>,
}

#[async_trait]
impl ChainIndexer for SVMIndexer {
    async fn new(rpc_url: String, db: &DatabaseConnection) -> eyre::Result<Self> {
        // Determine WebSocket URL
        let ws_url = if rpc_url.starts_with("http://") {
            rpc_url.replace("http://", "ws://")
        } else if rpc_url.starts_with("https://") {
            rpc_url.replace("https://", "wss://")
        } else if rpc_url.starts_with("ws://") || rpc_url.starts_with("wss://") {
            rpc_url
        } else {
            format!("wss://{}", rpc_url)
        };
        let program_id = "F2fAXerKhd4yq9NMbpucygsnWm77HDpm962TW8HupFHp";

        // Create channels
        let (data_sender, mut data_receiver) =
            mpsc::channel::<(String, Option<String>, String)>(100);
        let (event_sender, event_receiver) = mpsc::channel(100);

        // Spawn the subscription task
        tokio::spawn(async move {
            if let Err(e) = subscriber::subscribe_to_logs(&ws_url, program_id, data_sender).await {
                error!("Subscription error: {:?}", e);
            }
        });

        // Spawn the parsing task
        tokio::spawn(async move {
            while let Some((encoded_data, signature, event_type)) = data_receiver.recv().await {
                if let Some(event) = parser::parse_data(&encoded_data, signature, &event_type) {
                    if event_sender.send(event).await.is_err() {
                        error!("Failed to send parsed event to indexer");
                    }
                }
            }
        });

        Ok(Self {
            db: db.clone(),
            event_receiver,
        })
    }

    async fn run(&mut self) -> Result<()> {
        info!("SVM indexer running...");

        let mut latest_native_deposit_nonce = db::get_latest_native_token_deposit_nonce(&self.db)
            .await?
            .unwrap_or(0);
        let mut latest_spl_deposit_nonce = db::get_latest_spl_token_deposit_nonce(&self.db)
            .await?
            .unwrap_or(0);
        let mut latest_native_withdrawal_nonce = db::get_latest_native_withdrawal_nonce(&self.db) // Assuming this function exists
            .await?
            .unwrap_or(0);
        let mut latest_spl_withdrawal_nonce = db::get_latest_spl_withdrawal_nonce(&self.db) // Assuming this function exists
            .await?
            .unwrap_or(0);

        // Process incoming events from the channel
        while let Some(event) = self.event_receiver.recv().await {
            // Try to deserialize as DepositSuccessful
            if let Ok(deposit) = serde_json::from_value::<parser::DepositSuccessful>(event.clone())
            {
                let nonce = deposit.nonce;
                let is_native = deposit.l1_token == "11111111111111111111111111111111";

                if is_native {
                    if (nonce as i64) <= latest_native_deposit_nonce {
                        info!(
                            "Skipping native deposit with nonce {} (not newer than {})",
                            nonce, latest_native_deposit_nonce
                        );
                        continue;
                    }
                    db::insert_sol_deposit(&deposit, &self.db).await?;
                    latest_native_deposit_nonce = nonce as i64;
                    info!("Inserted native deposit with nonce: {}", nonce);
                } else {
                    if (nonce as i64) <= latest_spl_deposit_nonce {
                        info!(
                            "Skipping SPL deposit with nonce {} (not newer than {})",
                            nonce, latest_spl_deposit_nonce
                        );
                        continue;
                    }
                    db::insert_spl_deposit(&deposit, &self.db).await?;
                    latest_spl_deposit_nonce = nonce as i64;
                    info!("Inserted SPL deposit with nonce: {}", nonce);
                }
            } else if let Ok(withdrawal) =
                serde_json::from_value::<parser::ForcedWithdrawSuccessful>(event.clone())
            {
                let nonce = withdrawal.nonce;
                let is_native = withdrawal.l1_token == "11111111111111111111111111111111";

                if is_native {
                    if (nonce as i64) <= latest_native_withdrawal_nonce {
                        info!(
                            "Skipping native withdrawal with nonce {} (not newer than {})",
                            nonce, latest_native_withdrawal_nonce
                        );
                        continue;
                    }
                    db::insert_native_withdrawal(&withdrawal, &self.db).await?;
                    latest_native_withdrawal_nonce = nonce as i64;
                    info!("Inserted native withdrawal with nonce: {}", nonce);
                } else {
                    if (nonce as i64) <= latest_spl_withdrawal_nonce {
                        info!(
                            "Skipping SPL withdrawal with nonce {} (not newer than {})",
                            nonce, latest_spl_withdrawal_nonce
                        );
                        continue;
                    }
                    db::insert_spl_withdrawal(&withdrawal, &self.db).await?;
                    latest_spl_withdrawal_nonce = nonce as i64;
                    info!("Inserted SPL withdrawal with nonce: {}", nonce);
                }
            } else {
                error!("Failed to deserialize event into DepositSuccessful or ForcedWithdrawSuccessful");
            }
        }

        Ok(())
    }

    async fn chain_id(&self) -> Result<u64> {
        Ok(900)
    }
}
