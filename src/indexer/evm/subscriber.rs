use alloy::{providers::Provider, rpc::types::Filter};
use eyre::Result;
use futures_util::Stream;

pub async fn subscribe(
    provider: &dyn Provider,
) -> Result<impl Stream<Item = alloy::rpc::types::Log>> {
    let filter = Filter::new(); // TODO: Add contract addresses to the filter
    let subscription = provider.subscribe_logs(&filter).await?;
    Ok(subscription.into_stream())
}
