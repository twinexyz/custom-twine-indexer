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
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::IndexerConfig::load()?;

    let db_conn = database::connect::connect(&cfg.database.url).await?;
    info!("Connected to Indexer's DB");
    let blockscout_db_conn = database::connect::connect(&cfg.blockscout.url).await?;
    info!("Connected to Blockscout's DB");

    let db_client = DbClient::new(db_conn.clone(), Some(blockscout_db_conn.clone()));
    let arc_db = Arc::new(db_client);

    let twine_handler = TwineEventHandler::new(
        Arc::clone(&arc_db),
        cfg.twine.clone(),
        Arc::new(EvmProvider::new(
            &cfg.twine.common.http_rpc_url,
            cfg.twine.common.chain_id,
        )),
    );
    let l1_evm_handler = EthereumEventHandler::new(
        Arc::clone(&arc_db),
        cfg.l1s.ethereum.clone(),
        Arc::new(EvmProvider::new(
            &cfg.twine.common.http_rpc_url,
            cfg.twine.common.chain_id,
        )),
    );
    let solana_handler = SolanaEventHandler::new(
        Arc::clone(&arc_db),
        cfg.l1s.solana.clone(),
        Arc::new(EvmProvider::new(
            &cfg.twine.common.http_rpc_url,
            cfg.twine.common.chain_id,
        )),
    );

    let mut eth_indexer =
        EvmIndexer::new(l1_evm_handler, Arc::clone(&arc_db), cfg.settings.clone());
    let mut twine_indexer =
        EvmIndexer::new(twine_handler, Arc::clone(&arc_db), cfg.settings.clone());
    let mut solana_indexer =
        SolanaIndexer::new(solana_handler, Arc::clone(&arc_db), cfg.settings.clone());

    let twine_handle = tokio::spawn(async move {
        info!("starting twine indexer");
        twine_indexer.run().await
    });

    let eth_handle = tokio::spawn(async move {
        info!("starting eth indexer");
        eth_indexer.run().await
    });
    let solana_handle = tokio::spawn(async move {
        info!("starting solana indexer");
        solana_indexer.run().await
    });

    let (eth_result, twine_result, solana_result) =
        tokio::join!(eth_handle, twine_handle, solana_handle);

    let mut all_ok = true;

    info!("All indexer tasks have completed. Checking results...");

    // Check the result of the Ethereum indexer task
    match eth_result {
        Ok(Ok(_)) => info!("✅ Ethereum indexer finished successfully."),
        Ok(Err(e)) => {
            error!("❌ Ethereum indexer returned an application error: {:?}", e);
            all_ok = false;
        }
        Err(e) => {
            error!(
                "🚨 Ethereum indexer task panicked or was cancelled: {:?}",
                e
            );
            all_ok = false;
        }
    }

    // Check the result of the Twine indexer task
    match twine_result {
        Ok(Ok(_)) => info!("✅ Twine indexer finished successfully."),
        Ok(Err(e)) => {
            error!("❌ Twine indexer returned an application error: {:?}", e);
            all_ok = false;
        }
        Err(e) => {
            error!("🚨 Twine indexer task panicked or was cancelled: {:?}", e);
            all_ok = false;
        }
    }

    // Check the result of the Solana indexer task
    match solana_result {
        Ok(Ok(_)) => info!("✅ Solana indexer finished successfully."),
        Ok(Err(e)) => {
            error!("❌ Solana indexer returned an application error: {:?}", e);
            all_ok = false;
        }
        Err(e) => {
            error!("🚨 Solana indexer task panicked or was cancelled: {:?}", e);
            all_ok = false;
        }
    }

    if !all_ok {
        return Err(eyre::eyre!(
            "One or more indexers failed to run to completion."
        ));
    }

    info!("All services shut down gracefully.");
    Ok(())
}
