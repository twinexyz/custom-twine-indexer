mod indexer;
use crate::indexer::run_indexer;
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let rpc_url = std::env::var("RPC_WS_URL").unwrap_or_else(|_| "ws://localhost:8546".to_owned());
    run_indexer(rpc_url).await
}
