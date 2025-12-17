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
use tokio::signal;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

/// Represents an indexer task with its name and join handle
struct IndexerTask {
    name: String,
    handle: JoinHandle<Result<()>>,
}

/// Spawns an indexer task with consistent logging
fn spawn_indexer<I>(name: &str, mut indexer: I) -> IndexerTask
where
    I: ChainIndexer + Send + 'static,
    I::EventHandler: Send + Sync + 'static,
{
    let name_clone = name.to_string();
    let handle = tokio::spawn(async move {
        info!("Starting {} indexer", name_clone);
        let result = indexer.run().await;
        match &result {
            Ok(_) => info!("{} indexer completed successfully", name_clone),
            Err(e) => error!("{} indexer failed: {:?}", name_clone, e),
        }
        result
    });

    IndexerTask {
        name: name.to_string(),
        handle,
    }
}

/// Sets up database connections
async fn setup_databases(cfg: &config::IndexerConfig) -> Result<Arc<DbClient>> {
    let db_conn = database::connect::connect(&cfg.database.url).await?;
    info!("Connected to Indexer's DB");

    let blockscout_db_conn = database::connect::connect(&cfg.blockscout.url).await?;
    info!("Connected to Blockscout's DB");

    let db_client = DbClient::new(db_conn.clone(), Some(blockscout_db_conn.clone()));
    Ok(Arc::new(db_client))
}

/// Creates and spawns all indexer tasks
async fn spawn_all_indexers(
    cfg: &config::IndexerConfig,
    arc_db: Arc<DbClient>,
    twine_provider: Arc<EvmProvider>,
) -> Result<Vec<IndexerTask>> {
    let mut tasks = Vec::new();

    // Twine indexer
    let twine_handler = TwineEventHandler::new(
        Arc::clone(&arc_db),
        cfg.twine.clone(),
        twine_provider.clone(),
    );
    let twine_indexer = EvmIndexer::new(twine_handler, Arc::clone(&arc_db), cfg.settings.clone());
    tasks.push(spawn_indexer("Twine", twine_indexer));

    // Ethereum indexer
    let eth_handler = EthereumEventHandler::new(
        Arc::clone(&arc_db),
        cfg.l1s.ethereum.clone(),
        twine_provider.clone(),
    );
    let eth_indexer = EvmIndexer::new(eth_handler, Arc::clone(&arc_db), cfg.settings.clone());
    tasks.push(spawn_indexer("Ethereum", eth_indexer));

    // Arbitrum indexer
    let arbitrum_handler = EthereumEventHandler::new(
        Arc::clone(&arc_db),
        cfg.l1s.arbitrum.clone(),
        twine_provider.clone(),
    );
    let arbitrum_indexer =
        EvmIndexer::new(arbitrum_handler, Arc::clone(&arc_db), cfg.settings.clone());
    tasks.push(spawn_indexer("Arbitrum", arbitrum_indexer));

    // Base indexer
    let base_handler = EthereumEventHandler::new(
        Arc::clone(&arc_db),
        cfg.l1s.base.clone(),
        twine_provider.clone(),
    );
    let base_indexer = EvmIndexer::new(base_handler, Arc::clone(&arc_db), cfg.settings.clone());
    tasks.push(spawn_indexer("Base", base_indexer));

    // Solana indexer
    let solana_handler = SolanaEventHandler::new(
        Arc::clone(&arc_db),
        cfg.l1s.solana.clone(),
        twine_provider.clone(),
    );
    let solana_indexer =
        SolanaIndexer::new(solana_handler, Arc::clone(&arc_db), cfg.settings.clone());
    tasks.push(spawn_indexer("Solana", solana_indexer));

    Ok(tasks)
}

/// Waits for all indexer tasks to complete and checks their results
/// Also handles graceful shutdown on receiving termination signals
async fn wait_for_indexers(tasks: Vec<IndexerTask>) -> Result<()> {
    info!("Waiting for all indexer tasks to complete...");
    info!("Press Ctrl+C to initiate graceful shutdown");

    let task_count = tasks.len();
    let task_names: Vec<String> = tasks.iter().map(|t| t.name.clone()).collect();
    let task_handles: Vec<_> = tasks.into_iter().map(|t| t.handle).collect();

    // Spawn shutdown signal handler in the background
    let mut shutdown_handle = tokio::spawn(handle_shutdown_signal());

    // Wait for all tasks to complete, or shutdown signal
    let results = tokio::select! {
        results = async {
            let mut collected = Vec::new();
            for handle in task_handles {
                collected.push(handle.await);
            }
            collected
        } => {
            shutdown_handle.abort();
            results
        }
        _ = &mut shutdown_handle => {
            warn!("Shutdown signal received. Tasks will be cancelled when process exits.");
            // Return early - tasks will be dropped and cancelled
            return Err(eyre::eyre!("Shutdown signal received"));
        }
    };

    check_indexer_results(results, &task_names, false, task_count)?;
    Ok(())
}

/// Checks the results of indexer tasks and logs appropriately
fn check_indexer_results(
    results: Vec<Result<Result<()>, tokio::task::JoinError>>,
    task_names: &[String],
    shutdown_received: bool,
    task_count: usize,
) -> Result<()> {
    let mut failed_count = 0;
    for (idx, result) in results.into_iter().enumerate() {
        let task_name = task_names.get(idx).map(|s| s.as_str()).unwrap_or("Unknown");
        match result {
            Ok(Ok(_)) => {
                if !shutdown_received {
                    info!("✅ {} indexer finished successfully", task_name);
                }
            }
            Ok(Err(e)) => {
                error!(
                    "❌ {} indexer returned an application error: {:?}",
                    task_name, e
                );
                failed_count += 1;
            }
            Err(e) => {
                if e.is_cancelled() {
                    if shutdown_received {
                        warn!("{} indexer was cancelled during shutdown", task_name);
                    } else {
                        warn!("🚨 {} indexer task was cancelled: {:?}", task_name, e);
                        failed_count += 1;
                    }
                } else {
                    error!("🚨 {} indexer task panicked: {:?}", task_name, e);
                    failed_count += 1;
                }
            }
        }
    }

    if !shutdown_received && failed_count > 0 {
        warn!(
            "{} out of {} indexers failed to run to completion",
            failed_count, task_count
        );
        return Err(eyre::eyre!(
            "One or more indexers failed to run to completion"
        ));
    }

    if !shutdown_received {
        info!("All {} indexers shut down gracefully", task_count);
    }
    Ok(())
}

/// Handles shutdown signals (SIGINT, SIGTERM)
async fn handle_shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        }
        _ = terminate => {
            info!("Received SIGTERM signal");
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Initializing indexer service...");

    let cfg = config::IndexerConfig::load()?;
    let arc_db = setup_databases(&cfg).await?;

    let twine_provider = Arc::new(EvmProvider::new(
        &cfg.twine.common.http_rpc_url,
        cfg.twine.common.chain_id,
    ));

    let tasks = spawn_all_indexers(&cfg, arc_db, twine_provider).await?;
    info!("Spawned {} indexer tasks", tasks.len());

    wait_for_indexers(tasks).await
}
