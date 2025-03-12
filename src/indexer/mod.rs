mod evm;
mod twine;
mod svm;

use async_trait::async_trait;
use eyre::Result;
use sea_orm::DatabaseConnection;

use crate::config::Config;

#[async_trait]
pub trait ChainIndexer: Send + Sync {
    /// Constructs a new indexer.
    async fn new(rpc_url: String, db: &DatabaseConnection) -> Result<Self>
    where
        Self: Sized;

    /// Runs the indexer event loop.
    async fn run(&mut self) -> Result<()>;

    /// Returns the chain id.
    async fn chain_id(&self) -> Result<u64>;
}

pub async fn start_indexer(config: Config, db_conn: DatabaseConnection) -> Result<()> {
    // Create indexers
    let mut evm_indexer = evm::EVMIndexer::new(config.evm_rpc_url, &db_conn).await?;
    let mut svm_indexer = svm::SVMIndexer::new(config.svm_rpc_url, &db_conn).await?;
    // evm_indexer.run().await;

    // Run indexers concurrently with mutable references
    tokio::try_join!(evm_indexer.run(), svm_indexer.run())?;

    Ok(())
}
