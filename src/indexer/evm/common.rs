use crate::indexer::{MAX_RETRIES, RETRY_DELAY};
use super::EVMChain;
use alloy::{
    primitives::{address, Address},
    providers::{Provider, ProviderBuilder, WsConnect},
    rpc::types::{Filter, Log},
};
use eyre::{Report, Result};
use futures_util::Stream;
use tracing::{error, info, instrument};

#[instrument(skip_all, fields(CHAIN = %chain))]
pub async fn subscribe_stream(
    provider: &dyn Provider,
    contract_addresses: &[String],
    chain: EVMChain,
) -> Result<impl Stream<Item = Log>> {
    let addresses = contract_addresses
        .iter()
        .map(|addr| addr.parse::<Address>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| eyre::eyre!("Invalid address format: {}", e))?;

    let filter = Filter::new().address(addresses);
    info!("Creating log subscription");

    let subscription = provider.subscribe_logs(&filter).await?;
    Ok(subscription.into_stream())
}

#[instrument(skip_all, fields(CHAIN = %chain))]
pub async fn poll_missing_logs(
    provider: &dyn Provider,
    last_synced: u64,
    contract_addresses: &[String],
    chain: EVMChain,
) -> Result<Vec<Log>> {
    const MAX_BLOCKS_PER_REQUEST: u64 = 1000;

    let current_block = provider.get_block_number().await?;
    info!("Current block: {}", current_block);

    if last_synced == current_block {
        info!("Already synced to latest block");
        return Ok(Vec::new());
    }

    let addresses = contract_addresses
        .iter()
        .map(|addr| addr.parse::<Address>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| eyre::eyre!("Invalid address format: {}", e))?;

    let mut all_logs = Vec::new();
    let mut start_block = last_synced + 1;

    while start_block <= current_block {
        let end_block = (start_block + MAX_BLOCKS_PER_REQUEST - 1).min(current_block);
        info!(start_block, end_block, "Fetching block range");

        let filter = Filter::new()
            .select(start_block..=end_block)
            .address(addresses.clone());

        match provider.get_logs(&filter).await {
            Ok(logs) => {
                info!(log_count = logs.len(), "Fetched logs");
                all_logs.extend(logs);
            },
            Err(e) => {
                error!(error = %e, "Failed to fetch logs");
                return Err(e.into());
            }
        }

        start_block = end_block + 1;
    }

    info!(total_logs = all_logs.len(), "Completed log fetch");
    Ok(all_logs)
}

#[instrument(skip_all, fields(CHAIN = %chain))]
pub async fn create_ws_provider(
    ws_rpc_url: String,
    chain: EVMChain,
) -> Result<impl Provider> {
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
pub async fn create_http_provider(
    http_rpc_url: String,
    chain: EVMChain,
) -> Result<impl Provider> {
    let parsed_url = http_rpc_url.parse()
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
