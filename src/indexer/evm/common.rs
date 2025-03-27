use crate::indexer::{MAX_RETRIES, RETRY_DELAY};
use alloy::{
    primitives::{address, Address},
    providers::{Provider, ProviderBuilder, WsConnect},
    rpc::types::{Filter, Log},
};
use eyre::{Report, Result};
use futures_util::Stream;
use tracing::{error, info};

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
    info!("Current block is: {}", current_block);
    info!("Last synced block is: {}", last_synced);
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

pub async fn create_ws_provider(ws_rpc_url: String) -> Result<impl Provider> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match ProviderBuilder::new()
            .on_ws(WsConnect::new(&ws_rpc_url))
            .await
        {
            Ok(provider) => {
                info!("Connected to WS provider on attempt {}", attempt);
                return Ok(provider);
            }
            Err(e) => {
                error!("WS Attempt {} failed to connect: {}.", attempt, e);
                if attempt >= MAX_RETRIES {
                    error!("Exceeded maximum WS connection attempts.");
                    return Err(Report::from(e));
                }
                // Wait before retrying
                std::thread::sleep(RETRY_DELAY);
            }
        }
    }
}

pub async fn create_http_provider(http_rpc_url: String) -> Result<impl Provider> {
    let parsed_url = http_rpc_url.parse()?;
    let provider = ProviderBuilder::new().on_http(parsed_url);
    let mut attempt = 0;
    loop {
        attempt += 1;
        // Test if the provider is working by fetching the chain ID
        match provider.get_chain_id().await {
            Ok(chain_id) => {
                info!(
                    "Successfully connected to HTTP provider on attempt {}.",
                    attempt
                );
                return Ok(provider);
            }
            Err(e) => {
                error!(
                    "Attempt {} failed to connect to HTTP provider: {}",
                    attempt, e
                );
                if attempt >= MAX_RETRIES {
                    error!("Exceeded maximum connection attempts.");
                    return Err(eyre::Report::from(e));
                }
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }
}
