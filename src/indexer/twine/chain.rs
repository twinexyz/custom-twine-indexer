use alloy::{
    providers::Provider,
    rpc::types::{Filter, Log},
};
use eyre::Result;
use futures_util::Stream;
use tracing::info;

pub async fn subscribe_stream(
    provider: &dyn Provider,
) -> Result<impl Stream<Item = alloy::rpc::types::Log>> {
    let filter = Filter::new(); // TODO: Add contract addresses to the filter
    let subscription = provider.subscribe_logs(&filter).await?;
    Ok(subscription.into_stream())
}

pub async fn poll_missing_logs(provider: &dyn Provider, last_synced: u64) -> Result<Vec<Log>> {
    let current_block = provider.get_block_number().await?;
    info!("Current block is: {current_block}");
    info!("Last synced block is: {last_synced}");
    if last_synced == current_block {
        return Ok(Vec::new());
    }
    let filter = Filter::new().select((last_synced+1)..);
    let logs = provider.get_logs(&filter).await?;
    Ok(logs)
}
