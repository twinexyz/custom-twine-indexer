use std::sync::Arc;

use common::config::{self, LoadFromEnv};
use database::client::DbClient;
use evm::{
    ethereum::handlers::EthereumEventHandler, indexer::EvmIndexer,
    twine::handlers::TwineEventHandler,
};
use eyre::Result;
use svm::{handler::SolanaEventHandler, indexer::SolanaIndexer, SVMIndexer};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::IndexerConfig::from_env()?;

    let db_conn = database::connect::connect(&cfg.database.url).await?;
    info!("Connected to Indexer's DB");
    let blockscout_db_conn = database::connect::connect(&cfg.blockscout.url).await?;
    info!("Connected to Blockscout's DB");

    let db_client = DbClient::new(db_conn.clone(), blockscout_db_conn.clone());
    let arc_db = Arc::new(db_client);

    let twine_handler = TwineEventHandler::new(Arc::clone(&arc_db), cfg.twine.clone());
    let l1_evm_handler = EthereumEventHandler::new(Arc::clone(&arc_db), cfg.l1s.ethereum.clone());
    let solana_handler = SolanaEventHandler::new(Arc::clone(&arc_db), cfg.l1s.solana.clone());

    let eth_indexer = EvmIndexer::new(l1_evm_handler, Arc::clone(&arc_db));
    let twine_indexer = EvmIndexer::new(twine_handler, Arc::clone(&arc_db));

    // Create Indexers here
    let mut solana_indexer = SolanaIndexer::new(Arc::clone(&arc_db), solana_handler).await;

    let _ = solana_indexer.run().await;

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
