mod db;
mod parser;
mod subscriber;

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

        // Polling interval (e.g., every 10 seconds)
        let mut interval = time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await; // Wait for the next tick

            let deposits = subscriber::start_polling().await?;
            info!("SVM subscriber retrieved deposits: {:?}", deposits);

            // Insert each deposit into the database
            for deposit in deposits {
                db::insert_deposit(&deposit, &self.db).await?; // Pass reference with &
                info!(
                    "Inserted deposit with nonce: {}",
                    deposit.deposit_message.nonce
                );
            }
        }
    }

    async fn chain_id(&self) -> Result<u64> {
        Ok(900) // From your response, chain_id is 900
    }
}
