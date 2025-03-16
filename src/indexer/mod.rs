mod evm;
mod svm;
mod twine;

use async_trait::async_trait;
use eyre::Result;
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;
use tracing::{error, info};

use crate::config::Config;

#[async_trait]
pub trait ChainIndexer: Send + Sync {
    async fn new(rpc_url: String, db: &DatabaseConnection) -> Result<Self>
    where
        Self: Sized;
    async fn run(&mut self) -> Result<()>;
    async fn chain_id(&self) -> Result<u64>;
}
pub async fn start_indexer(
    config: Config,
    db_conn: DatabaseConnection,
) -> Result<(JoinHandle<Result<()>>, JoinHandle<Result<()>>)> {
    let mut evm_indexer = evm::EVMIndexer::new(config.evm_rpc_url, &db_conn).await?;
    let mut svm_indexer = svm::SVMIndexer::new(config.svm_rpc_url, &db_conn).await?;

    let evm_handle = tokio::spawn(async move {
        info!("Starting EVM indexer");
        evm_indexer.run().await
    });

    let svm_handle = tokio::spawn(async move {
        info!("Starting SVM indexer");
        svm_indexer.run().await // Await directly in the async context
    });

    Ok((evm_handle, svm_handle))
}
