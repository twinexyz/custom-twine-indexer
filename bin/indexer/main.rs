// bin/indexer/main.rs
use eyre::Result;
use tracing::info;
use twine_indexer::{config, db, indexer};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::Config::from_env()?;
    let db_conn = db::connect(&cfg.database_url).await?;
    info!("Connected to Database");

    let (evm_handle, svm_handle) = indexer::start_indexer(cfg, db_conn).await?;

    // Wait for both indexers to complete
    tokio::try_join!(evm_handle, svm_handle)?;

    Ok(())
}
