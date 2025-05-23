use std::sync::Arc;

use common::config::{self, LoadFromEnv as _};
use da::{celestia::provider::CelestiaProvider, db::DbClient};
use eyre::{Ok, Result};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cfg = config::DaIndexerConfig::from_env("da_indexer".to_string())?;

    let db_conn = database::connect::connect(&cfg.blockscout.url).await?;
    info!("Connected to Blockscout's DB");

    let db_client = DbClient::new(&db_conn);

    let arc_db = Arc::new(db_client);
    let provider = CelestiaProvider::new(cfg.celestia, arc_db).await;

    let _ = provider.run_indexer().await;

    Ok(())
}
