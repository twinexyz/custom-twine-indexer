use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use evm::EVMIndexer;
use tracing::info;

mod evm;

async fn get_provider(rpc_url: String) -> eyre::Result<impl Provider> {
    let ws = WsConnect::new(&rpc_url);
    let provider = ProviderBuilder::new().on_ws(ws).await?;
    let chain_id = provider.get_chain_id().await?;
    info!("Connected to blockchain. Chain ID: {chain_id}");
    Ok(provider)
}

pub async fn start_indexer(
    rpc_url: String,
    db_conn: sea_orm::DatabaseConnection,
) -> eyre::Result<()> {
    let evm_provider = get_provider(rpc_url).await?;
    // subscriber::run_indexer(provider, db_conn).await
    let evm_indexer = EVMIndexer::new(evm_provider, db_conn);
    evm_indexer.run().await
}
