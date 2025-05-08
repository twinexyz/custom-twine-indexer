use std::sync::Arc;

use common::{
    config::{self, LoadFromEnv as _},
    db,
};
use da::{celestia::provider::CelestiaProvider, db::DbClient};
use eyre::{Ok, Result};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cfg = config::DAConfig::from_env()?;

    let db_conn = db::connect(&cfg.blockscout_database_url).await?;
    info!("Connected to Blockscout's DB");

    let db_client = DbClient::new(&db_conn);

    let arc_db = Arc::new(db_client);
    let provider = CelestiaProvider::new(cfg.celestia, arc_db).await;

    let _ = provider.run_indexer().await;

    Ok(())
}
