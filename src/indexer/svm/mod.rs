mod db;
mod parser;
mod subscriber;

use super::ChainIndexer;
use async_trait::async_trait;
use eyre::{eyre, Result};
use sea_orm::DatabaseConnection;
use serde_json::Value;
use std::env;
use tokio::sync::mpsc::{self, Receiver};
use tracing::{error, info};

pub struct SVMIndexer {
    db: DatabaseConnection,
    event_receiver: Receiver<Value>,
    subscriber: subscriber::LogSubscriber,
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

        let twine_chain_program_id = env::var("TWINE_CHAIN_PROGRAM_ID")
            .map_err(|_| eyre!("TWINE_CHAIN_PROGRAM_ID environment variable not set"))?;
        let tokens_gateway_program_id = env::var("TOKENS_GATEWAY_PROGRAM_ID")
            .map_err(|_| eyre!("TOKENS_GATEWAY_PROGRAM_ID environment variable not set"))?;
        info!("Using twine_chain_program_id: {}", twine_chain_program_id);
        info!(
            "Using tokens_gateway_program_id: {}",
            tokens_gateway_program_id
        );

        let (data_sender, mut data_receiver) =
            mpsc::channel::<(String, Option<String>, String)>(100);
        let (event_sender, event_receiver) = mpsc::channel(100);

        let subscriber = subscriber::LogSubscriber::new(
            &ws_url,
            &twine_chain_program_id,
            &tokens_gateway_program_id,
            data_sender,
        );

        // Spawn the subscription task
        tokio::spawn({
            let subscriber = subscriber.clone();
            async move {
                if let Err(e) = subscriber.run().await {
                    error!("Subscription error: {:?}", e);
                }
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
            subscriber,
        })
    }

    async fn run(&mut self) -> Result<()> {
        info!("SVM indexer running...");

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
            if let Ok(deposit) = serde_json::from_value::<parser::DepositSuccessful>(event.clone())
            {
                let nonce = deposit.nonce as i64;
                if deposit.chain_id as i64 != chain_id {
                    info!(
                        "Skipping deposit with chain_id {} (expected {})",
                        deposit.chain_id, chain_id
                    );
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

                if is_native {
                    db::insert_sol_deposit(&deposit, &self.db).await?;
                    info!("✅ Inserted native deposit with nonce: {}", nonce);
                } else {
                    db::insert_spl_deposit(&deposit, &self.db).await?;
                    info!("✅ Inserted SPL deposit with nonce: {}", nonce);
                }
                latest_deposit_nonce = nonce;
            } else if let Ok(withdrawal) =
                serde_json::from_value::<parser::ForcedWithdrawSuccessful>(event.clone())
            {
                let nonce = withdrawal.nonce as i64;
                if withdrawal.chain_id as i64 != chain_id {
                    info!(
                        "Skipping withdrawal with chain_id {} (expected {})",
                        withdrawal.chain_id, chain_id
                    );
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
                    info!("✅ Inserted native withdrawal with nonce: {}", nonce);
                } else {
                    db::insert_spl_withdrawal(&withdrawal, &self.db).await?;
                    info!("✅ Inserted SPL withdrawal with nonce: {}", nonce);
                }
                latest_withdrawal_nonce = nonce;
            } else if let Ok(native_success) =
                serde_json::from_value::<parser::NativeWithdrawalSuccessful>(event.clone())
            {
                let nonce = native_success.nonce as i64;
                if native_success.chain_id as i64 != chain_id {
                    info!(
                        "Skipping native withdrawal successful with chain_id {} (expected {})",
                        native_success.chain_id, chain_id
                    );
                    continue;
                }
                if nonce <= latest_withdrawal_nonce {
                    info!(
                        "Skipping native withdrawal successful with nonce {} (not newer than {})",
                        nonce, latest_withdrawal_nonce
                    );
                    continue;
                }

                db::insert_native_withdrawal_successful(&native_success, &self.db).await?;
                info!(
                    "✅ Inserted native withdrawal successful with nonce: {}",
                    nonce
                );
                latest_withdrawal_nonce = nonce;
            } else if let Ok(spl_success) =
                serde_json::from_value::<parser::SplWithdrawalSuccessful>(event.clone())
            {
                let nonce = spl_success.nonce as i64;
                if spl_success.chain_id as i64 != chain_id {
                    info!(
                        "Skipping SPL withdrawal successful with chain_id {} (expected {})",
                        spl_success.chain_id, chain_id
                    );
                    continue;
                }
                if nonce <= latest_withdrawal_nonce {
                    info!(
                        "Skipping SPL withdrawal successful with nonce {} (not newer than {})",
                        nonce, latest_withdrawal_nonce
                    );
                    continue;
                }

                db::insert_spl_withdrawal_successful(&spl_success, &self.db).await?;
                info!(
                    "✅ Inserted SPL withdrawal successful with nonce: {}",
                    nonce
                );
                latest_withdrawal_nonce = nonce;
            } else {
                error!("Failed to deserialize event: {:?}", event);
            }
        }

        info!("SVM indexer stopped");
        self.subscriber.shutdown();
        Ok(())
    }

    async fn chain_id(&self) -> Result<u64> {
        Ok(900)
    }
}
