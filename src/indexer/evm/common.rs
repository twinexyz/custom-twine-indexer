use super::EVMChain;
use crate::indexer::{MAX_RETRIES, RETRY_DELAY};
use alloy::{
    primitives::{address, Address},
    providers::{Provider, ProviderBuilder, WsConnect},
    rpc::types::{Filter, Log},
};
use eyre::{Report, Result};
use futures_util::Stream;
use tracing::{debug, error, info, instrument};

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

    let subscription = provider.subscribe_logs(&filter).await?;
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
    let current_block = provider.get_block_number().await?;

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

    let total_blocks = current_block - last_synced;
    let mut blocks_processed = 0;

    while start_block <= current_block {
        let end_block = (start_block + max_blocks_per_request - 1).min(current_block);
        let filter = Filter::new()
            .select(start_block..=end_block)
            .events(events)
            .address(addresses.clone());

        match provider.get_logs(&filter).await {
            Ok(logs) => {
                if !logs.is_empty() {
                    debug!(
                        "Fetched {} logs for blocks {}-{}",
                        logs.len(),
                        start_block,
                        end_block
                    );
                }
                all_logs.extend(logs);
            }
            Err(e) => {
                error!(error = %e, "Failed to fetch logs for blocks {}-{}", start_block, end_block);
                return Err(e.into());
            }
        }
        start_block = end_block + 1;
    }

    info!(
        "Completed log sync: {} logs fetched across {} blocks",
        all_logs.len(),
        total_blocks
    );

    Ok(all_logs)
}

#[instrument(skip_all, fields(CHAIN = %chain))]
pub async fn create_ws_provider(ws_rpc_url: String, chain: EVMChain) -> Result<impl Provider> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        info!(attempt, "Connecting to WS provider");

        match ProviderBuilder::new()
            .on_ws(WsConnect::new(&ws_rpc_url))
            .await
        {
            Ok(provider) => {
                info!("WS connection established");
                return Ok(provider);
            }
            Err(e) => {
                error!(error = %e, "WS connection failed");

                if attempt >= MAX_RETRIES {
                    error!("Max connection attempts reached");
                    return Err(Report::from(e));
                }

                info!(retry_in = ?RETRY_DELAY, "Retrying connection");
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }
}

#[instrument(skip_all, fields(CHAIN = %chain))]
pub async fn create_http_provider(http_rpc_url: String, chain: EVMChain) -> Result<impl Provider> {
    let parsed_url = http_rpc_url
        .parse()
        .map_err(|e| eyre::eyre!("Invalid HTTP URL: {}", e))?;

    let provider = ProviderBuilder::new().on_http(parsed_url);
    let mut attempt = 0;

    loop {
        attempt += 1;
        info!(attempt, "Connecting to HTTP provider");

        match provider.get_chain_id().await {
            Ok(chain_id) => {
                info!(chain_id, "HTTP connection verified");
                return Ok(provider);
            }
            Err(e) => {
                error!(error = %e, "HTTP connection failed");

                if attempt >= MAX_RETRIES {
                    error!("Max connection attempts reached");
                    return Err(eyre::Report::from(e));
                }

                info!(retry_in = ?RETRY_DELAY, "Retrying connection");
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }
}
