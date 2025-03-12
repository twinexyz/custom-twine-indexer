mod db;
mod parser;
mod subscriber;

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
        let ws_url = if rpc_url.starts_with("http://") {
            rpc_url.replace("http://", "ws://")
        } else if rpc_url.starts_with("https://") {
            rpc_url.replace("https://", "wss://")
        } else if rpc_url.starts_with("ws://") || rpc_url.starts_with("wss://") {
            rpc_url
        } else {
            format!("wss://{}", rpc_url)
        };
        let program_id = "9Q9FQZWzbEznXaFChEuipbHLWiGvvYrv9csKDK1sJz2N";

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

        // Fetch the latest nonces for deposits and withdrawals for chain_id
        let chain_id = self.chain_id().await? as i64;
        let mut latest_deposit_nonce = db::get_latest_deposit_nonce(&self.db, chain_id)
            .await?
            .unwrap_or(0);
        let mut latest_withdrawal_nonce = db::get_latest_withdrawal_nonce(&self.db, chain_id)
            .await?
            .unwrap_or(0);

        info!(
            "Starting with latest deposit nonce: {}, withdrawal nonce: {} for chain_id: {}",
            latest_deposit_nonce, latest_withdrawal_nonce, chain_id
        );

        while let Some(event) = self.event_receiver.recv().await {
            info!("Received raw event: {:?}", event);

            if let Ok(deposit) = serde_json::from_value::<parser::DepositSuccessful>(event.clone())
            {
                let nonce = deposit.nonce as i64;
                if deposit.chain_id as i64 != chain_id {
                    info!("Skipping deposit with chain_id {} (expected {})", deposit.chain_id, chain_id);
                    continue;
                }
                if nonce <= latest_deposit_nonce {
                    info!(
                        "Skipping deposit with nonce {} (not newer than {})",
                        nonce, latest_deposit_nonce
                    );
                    continue;
                }

                let is_native = deposit.l1_token == "11111111111111111111111111111111";
                info!(
                    "Parsed deposit event: native={}, nonce={}",
                    is_native, nonce
                );

                if is_native {
                    db::insert_sol_deposit(&deposit, &self.db).await?;
                    info!("Inserted native deposit with nonce: {}", nonce);
                } else {
                    db::insert_spl_deposit(&deposit, &self.db).await?;
                    info!("Inserted SPL deposit with nonce: {}", nonce);
                }
                latest_deposit_nonce = nonce;
            } else if let Ok(withdrawal) =
                serde_json::from_value::<parser::ForcedWithdrawSuccessful>(event.clone())
            {
                let nonce = withdrawal.nonce as i64;
                if withdrawal.chain_id as i64 != chain_id {
                    info!("Skipping withdrawal with chain_id {} (expected {})", withdrawal.chain_id, chain_id);
                    continue;
                }
                if nonce <= latest_withdrawal_nonce {
                    info!(
                        "Skipping withdrawal with nonce {} (not newer than {})",
                        nonce, latest_withdrawal_nonce
                    );
                    continue;
                }

                let is_native = withdrawal.l1_token == "11111111111111111111111111111111";
                info!(
                    "Parsed withdrawal event: native={}, nonce={}",
                    is_native, nonce
                );

                if is_native {
                    db::insert_native_withdrawal(&withdrawal, &self.db).await?;
                    info!("Inserted native withdrawal with nonce: {}", nonce);
                } else {
                    db::insert_spl_withdrawal(&withdrawal, &self.db).await?;
                    info!("Inserted SPL withdrawal with nonce: {}", nonce);
                }
                latest_withdrawal_nonce = nonce;
            } else {
                error!("Failed to deserialize event: {:?}", event);
            }
        }

        info!("SVM indexer stopped");
        Ok(())
    }

    async fn chain_id(&self) -> Result<u64> {
        Ok(900)
    }
}