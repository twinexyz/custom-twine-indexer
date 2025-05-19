use common::{
    config::{self, LoadFromEnv},
};
use database::client::DbClient;
use evm::EthereumIndexer;
use eyre::{Result};
use svm::SVMIndexer;
use tracing::{info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::IndexerConfig::from_env()?;





    let db_conn = db::connect(&cfg.database_url).await?;
    info!("Connected to Indexer's DB");
    let blockscout_db_conn = db::connect(&cfg.blockscout_database_url).await?;
    info!("Connected to Blockscout's DB");

    let db_client = DbClient::new(db_conn.clone(), blockscout_db_conn.clone());

    // Create Indexers here

    let solana_indexer = SVMIndexer::new(cfg.l1s.solana, db_client).await?;
    let eth_indexer = EthereumIndexer::new(cfg.l1s.ethereum, db_client).await?;






    // let handles = start_indexer(cfg, db_conn, blockscout_db_conn)
    //     .await
    //     .wrap_err("Failed to start indexers")?;

    // for handle in handles {
    //     match handle.await {
    //         Ok(inner_result) => {
    //             if let Err(e) = inner_result {
    //                 error!("Indexer task failed: {:?}", e);
    //             }
    //         }
    //         Err(e) => {
    //             if e.is_panic() {
    //                 error!("A task panicked: {:?}", e);
    //             } else {
    //                 error!("A task was cancelled: {:?}", e);
    //             }
    //         }
    //     }
    // }

    Ok(())
}
