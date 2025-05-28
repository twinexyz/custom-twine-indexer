use std::sync::Arc;

use common::config::{self, LoadFromEnv};
use database::client::DbClient;
use evm::{
    ethereum::handlers::EthereumEventHandler, indexer::EvmIndexer, provider::EvmProvider,
    twine::handlers::TwineEventHandler,
};
use eyre::Result;
use generic_indexer::indexer::ChainIndexer;
use svm::{handler::SolanaEventHandler, indexer::SolanaIndexer};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::IndexerConfig::from_env("indexer".to_string())?;

    let db_conn = database::connect::connect(&cfg.database.url).await?;
    info!("Connected to Indexer's DB");
    let blockscout_db_conn = database::connect::connect(&cfg.blockscout.url).await?;
    info!("Connected to Blockscout's DB");

    let db_client = DbClient::new(db_conn.clone(), blockscout_db_conn.clone());
    let arc_db = Arc::new(db_client);

    let twine_provider = EvmProvider::new(
        &cfg.twine.common.http_rpc_url,
        &cfg.twine.common.ws_rpc_url,
        cfg.twine.common.chain_id,
    )
    .await
    .unwrap();

    let twine_handler = TwineEventHandler::new(Arc::clone(&arc_db), cfg.twine.clone());
    let l1_evm_handler = EthereumEventHandler::new(
        Arc::clone(&arc_db),
        cfg.l1s.ethereum.clone(),
        twine_provider.clone(),
    );
    let solana_handler = SolanaEventHandler::new(
        Arc::clone(&arc_db),
        cfg.l1s.solana.clone(),
        twine_provider.clone(),
    );

    let mut eth_indexer = EvmIndexer::new(l1_evm_handler).await?;
    let mut twine_indexer = EvmIndexer::new(twine_handler).await?;
    let mut solana_indexer = SolanaIndexer::new(Arc::clone(&arc_db), solana_handler).await?;

    let twine_handle = tokio::spawn(async move {
        info!("starting twine indexer");
        // twine_indexer.run().await
    });

    let eth_handle = tokio::spawn(async move {
        info!("starting eth indexer");
        eth_indexer.run().await
    });
    let solana_handle = tokio::spawn(async move {
        info!("starting solana indexer");
        // solana_indexer.run().await
    });

    let _ = tokio::join!(eth_handle, twine_handle, solana_handle);

    Ok(())

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
}
