//- Only a comment
mod evm;

use async_trait::async_trait;
use eyre::Result;
use sea_orm::DatabaseConnection;

use crate::config::Config;

#[async_trait]
pub trait ChainIndexer: Send + Sync {
    /// Constructs a new indexer.
    async fn new(rpc_url: String, db: DatabaseConnection) -> Result<Self>
    where
        Self: Sized;

    /// Runs the indexer event loop.
    async fn run(&self) -> Result<()>;

    /// Returns the chain id.
    async fn chain_id(&self) -> Result<u64>;
}

pub async fn start_indexer(config: Config, db_conn: DatabaseConnection) -> Result<()> {
    let evm_indexer = evm::EVMIndexer::new(config.evm_rpc_url, db_conn).await?;
    evm_indexer.run().await
}
