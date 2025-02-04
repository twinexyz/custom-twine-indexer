mod config;
mod indexer;
mod db;
mod entities;

use crate::indexer::run_indexer;
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::Config::from_env()?;
    let db_conn = db::connect(&cfg.database_url).await?;
    tracing::info!("Connected to Postgres at {}", cfg.database_url);
    run_indexer(cfg.rpc_url, db_conn).await
}
