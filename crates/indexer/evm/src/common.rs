use super::EVMChain;
use alloy::{
    primitives::{address, Address},
    providers::{Provider, ProviderBuilder, WsConnect},
    rpc::types::{Filter, Log},
};
use common::indexer::{MAX_RETRIES, RETRY_DELAY};
use eyre::{Report, Result};
use futures_util::Stream;
use tokio::time::sleep;
use tracing::{debug, info, instrument};

pub async fn with_retry<F, Fut, T>(mut operation: F) -> Result<T, eyre::Report>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, eyre::Report>>,
{
    let mut attempt = 0;

    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempt += 1;
                if attempt >= MAX_RETRIES {
                    return Err(e);
                }
                tracing::warn!(attempt, "Operation failed, retrying in {:?}", RETRY_DELAY);
                sleep(RETRY_DELAY).await;
            }
        }
    }
}

#[instrument(skip_all, fields(CHAIN = %chain))]
pub async fn subscribe_stream(
    provider: &dyn Provider,
    contract_addresses: &[String],
    chain: EVMChain,
) -> Result<impl Stream<Item = Log>> {
    let events = chain.get_event_signatures();
    let addresses = contract_addresses
        .iter()
        .map(|addr| addr.parse::<Address>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| eyre::eyre!("Invalid address format: {}", e))?;

    let filter = Filter::new().address(addresses).events(events);
    info!("Creating log subscription");

    let subscription = with_retry(|| async {
        provider
            .subscribe_logs(&filter)
            .await
            .map_err(eyre::Report::from)
    })
    .await?;

    Ok(subscription.into_stream())
}

#[instrument(skip_all, fields(CHAIN = %chain))]
pub async fn poll_missing_logs(
    provider: &dyn Provider,
    last_synced: u64,
    max_blocks_per_request: u64,
    contract_addresses: &[String],
    chain: EVMChain,
) -> Result<Vec<Log>> {
    let current_block = with_retry(|| async {
        provider
            .get_block_number()
            .await
            .map_err(eyre::Report::from)
    })
    .await?;

    if last_synced == current_block {
        debug!("Already synced to latest block {}", current_block);
        return Ok(Vec::new());
    }

    info!(
        "Starting log sync from block {} to {} (max blocks per request: {})",
        last_synced + 1,
        current_block,
        max_blocks_per_request
    );

    let events = chain.get_event_signatures();
    let addresses = contract_addresses
        .iter()
        .map(|addr| addr.parse::<Address>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| eyre::eyre!("Invalid address format: {}", e))?;

    let mut all_logs = Vec::new();
    let mut start_block = last_synced + 1;

    while start_block <= current_block {
        let end_block = (start_block + max_blocks_per_request - 1).min(current_block);
        let filter = Filter::new()
            .select(start_block..=end_block)
            .events(events)
            .address(addresses.clone());

        let logs =
            with_retry(|| async { provider.get_logs(&filter).await.map_err(eyre::Report::from) })
                .await?;

        if !logs.is_empty() {
            debug!(
                "Fetched {} logs for blocks {}-{}",
                logs.len(),
                start_block,
                end_block
            );
        }
        all_logs.extend(logs);
        start_block = end_block + 1;
    }

    info!(
        "Completed log sync: {} logs fetched across {} blocks",
        all_logs.len(),
        current_block - last_synced
    );

    Ok(all_logs)
}

#[instrument(skip_all, fields(CHAIN = %chain))]
pub async fn create_ws_provider(ws_rpc_url: String, chain: EVMChain) -> Result<impl Provider> {
    let provider = with_retry(|| async {
        ProviderBuilder::new()
            .on_ws(WsConnect::new(&ws_rpc_url))
            .await
            .map_err(eyre::Report::from)
    })
    .await?;

    info!("WS connection established");
    Ok(provider)
}

#[instrument(skip_all, fields(CHAIN = %chain))]
pub async fn create_http_provider(http_rpc_url: String, chain: EVMChain) -> Result<impl Provider> {
    let parsed_url = http_rpc_url
        .parse()
        .map_err(|e| eyre::eyre!("Invalid HTTP URL: {}", e))?;

    let provider = ProviderBuilder::new().on_http(parsed_url);

    let chain_id =
        with_retry(|| async { provider.get_chain_id().await.map_err(eyre::Report::from) }).await?;

    info!(chain_id, "HTTP connection verified");
    Ok(provider)
}
