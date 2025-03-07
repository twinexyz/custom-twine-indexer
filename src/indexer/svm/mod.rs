mod db;
mod parser;
mod subscriber;

use super::ChainIndexer;
use async_trait::async_trait;
use eyre::Result;
use sea_orm::DatabaseConnection;
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
        let deposits = subscriber::poll_deposits().await?;
        info!("SVM subscriber retrieved deposits: {:?}", deposits);

        // Example: Store deposits in the database (implement as needed)
        // db::store_deposits(&self.db, &deposits).await?;

        Ok(())
    }

    async fn chain_id(&self) -> Result<u64> {
        Ok(1) // Replace with actual chain ID logic if needed
    }
}
