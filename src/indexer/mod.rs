mod evm;

pub async fn start_indexer(
    rpc_url: String,
    db_conn: sea_orm::DatabaseConnection,
) -> eyre::Result<()> {
    let evm_indexer = evm::EVMIndexer::new(rpc_url, db_conn).await?;
    evm_indexer.run().await
}
