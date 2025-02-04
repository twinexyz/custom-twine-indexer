mod config;
mod indexer;

use crate::indexer::run_indexer;
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::Config::from_env();
    run_indexer(cfg?.rpc_url).await
}
