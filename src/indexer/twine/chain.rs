use alloy::{
    primitives::{address, Address},
    providers::Provider,
    rpc::types::{Filter, Log},
};
use eyre::Result;
use futures_util::Stream;
use tracing::info;

pub async fn subscribe_stream(
    provider: &dyn Provider,
    contract_addresses: &[String],
) -> Result<impl Stream<Item = Log>> {
    let addresses = contract_addresses
        .into_iter()
        .map(|addr| addr.parse::<Address>().unwrap())
        .collect::<Vec<Address>>();
    let filter = Filter::new().address(addresses);
    let subscription = provider.subscribe_logs(&filter).await?;
    Ok(subscription.into_stream())
}

pub async fn poll_missing_logs(
    provider: &dyn Provider,
    last_synced: u64,
    contract_addresses: &[String],
) -> Result<Vec<Log>> {
    let current_block = provider.get_block_number().await?;
    info!("Current block is: {current_block}");
    info!("Last synced block is: {last_synced}");
    if last_synced == current_block {
        return Ok(Vec::new());
    }

    let addresses = contract_addresses
        .into_iter()
        .map(|addr| addr.parse::<Address>().unwrap())
        .collect::<Vec<Address>>();
    let mut filter = Filter::new()
        .select((last_synced + 1)..=(current_block))
        .address(addresses);
    let logs = provider.get_logs(&filter).await?;
    Ok(logs)
}
