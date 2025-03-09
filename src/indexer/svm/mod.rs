mod db;
mod parser;
mod subscriber;
mod utils;

use super::ChainIndexer;
use async_trait::async_trait;
use eyre::Result;
use sea_orm::DatabaseConnection;
use tokio::time::{self, Duration};
use tracing::info;

pub struct SVMIndexer {
    db: DatabaseConnection,
}

#[async_trait]
impl ChainIndexer for SVMIndexer {
    async fn new(rpc_url: String, db: &DatabaseConnection) -> eyre::Result<Self> {
        Ok(Self { db: db.clone() })
    }

    async fn run(&self) -> Result<()> {
        info!("SVM indexer running...");
        let mut interval = time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;

            let latest_nonce = db::get_latest_native_token_deposit_nonce(&self.db).await?;
            info!("Latest nonce in DB: {:?}", latest_nonce);

            let deposits = subscriber::start_polling().await?;
            info!("SVM subscriber retrieved deposits: {:?}", deposits);

            for deposit in deposits {
                if let Some(latest) = latest_nonce {
                    if deposit.deposit_message.nonce <= latest {
                        info!(
                            "Skipping deposit with nonce {} (not newer than {})",
                            deposit.deposit_message.nonce, latest
                        );
                        continue;
                    }
                }
                db::insert_sol_deposit(&deposit, &self.db).await?;
                info!(
                    "Inserted deposit with nonce: {}",
                    deposit.deposit_message.nonce
                );
            }
        }
    }

    async fn chain_id(&self) -> Result<u64> {
        Ok(900)
    }
}
