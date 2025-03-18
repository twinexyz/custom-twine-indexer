mod evm;
mod svm;
mod twine;

use crate::config::Config;
use async_trait::async_trait;
use eyre::Result;
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;
use tracing::{error, info};

#[async_trait]
pub trait ChainIndexer: Send + Sync {
    async fn new(rpc_url: String, db: &DatabaseConnection) -> Result<Self>
    where
        Self: Sized;
    async fn run(&mut self) -> Result<()>;
    async fn chain_id(&self) -> Result<u64>;
}

macro_rules! create_and_spawn_indexer {
    ($type:ty, $rpc_url:expr, $db_conn:expr, $name:expr) => {{
        let mut indexer = <$type>::new($rpc_url, &$db_conn).await?;
        tokio::spawn(async move {
            info!("Starting {} indexer", $name);
            indexer.run().await
        })
    }};
}

pub async fn start_indexer(
    config: Config,
    db_conn: DatabaseConnection,
) -> Result<(JoinHandle<Result<()>>, JoinHandle<Result<()>>)> {
    let evm_handle = create_and_spawn_indexer!(evm::EVMIndexer, config.evm_rpc_url, db_conn, "EVM");
    let twine_handle =
        create_and_spawn_indexer!(twine::TwineIndexer, config.twine_rpc_url, db_conn, "Twine");
    // let svm_handle = create_and_spawn_indexer!(svm::SVMIndexer, config.svm_rpc_url, db_conn, "SVM");

    Ok((evm_handle, twine_handle))
}
