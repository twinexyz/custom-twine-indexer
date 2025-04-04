use eyre::{Result, WrapErr};
use tracing::{error, info};
use twine_indexer::{config, db, indexer};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::IndexerConfig::from_env()?;
    let db_conn = db::connect(&cfg.database_url).await?;
    info!("Connected to Indexer's DB");
    let blockscout_db_conn = db::connect(&cfg.blockscout_database_url).await?;
    info!("Connected to Blockscout's DB");

    let handles = indexer::start_indexer(cfg, db_conn, blockscout_db_conn)
        .await
        .wrap_err("Failed to start indexers")?;

    for handle in handles {
        match handle.await {
            Ok(inner_result) => {
                if let Err(e) = inner_result {
                    error!("Indexer task failed: {:?}", e);
                }
            }
            Err(e) => {
                if e.is_panic() {
                    error!("A task panicked: {:?}", e);
                } else {
                    error!("A task was cancelled: {:?}", e);
                }
            }
        }
    }

    Ok(())
}
