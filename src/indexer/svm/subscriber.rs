use anchor_client::{
    solana_client::rpc_response::RpcConfirmedTransactionStatusWithSignature, Client, Cluster,
};
use eyre::Result;
use futures_util::{SinkExt, Stream, StreamExt};
use serde_json::json;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info};

use super::{MAX_RETRIES, RETRY_DELAY};
use solana_transaction_status_client_types::UiTransactionEncoding;

pub async fn subscribe_stream(
    ws_url: &str,
    twine_chain_id: &str,
    tokens_gateway_id: &str,
) -> Result<impl Stream<Item = (Vec<String>, Option<String>)>> {
    let (data_sender, data_receiver) = tokio::sync::mpsc::channel(100);

    let ws_url_owned = ws_url.to_string();
    let twine_chain_id_owned = twine_chain_id.to_string();
    let tokens_gateway_id_owned = tokens_gateway_id.to_string();

    info!(
        "Spawning subscription task for twine_chain_id: {}",
        twine_chain_id_owned
    );
    tokio::spawn(subscribe_to_single_program(
        ws_url_owned.clone(),
        twine_chain_id_owned.clone(),
        data_sender.clone(),
    ));

    info!(
        "Spawning subscription task for tokens_gateway_id: {}",
        tokens_gateway_id_owned
    );
    tokio::spawn(subscribe_to_single_program(
        ws_url_owned,
        tokens_gateway_id_owned.clone(),
        data_sender,
    ));

    Ok(ReceiverStream::new(data_receiver))
}

pub async fn poll_missing_slots(
    rpc_url: &str,
    twine_chain_id: &str,
    tokens_gateway_id: &str,
    last_synced: u64,
) -> Result<Vec<(Vec<String>, Option<String>)>> {
    let mut retries = 0;
    let current_slot = loop {
        match tokio::task::spawn_blocking({
            let rpc_url_clone = rpc_url.to_string();
            let twine_chain_id_clone = twine_chain_id.to_string();
            move || {
                let client: Client<Arc<anchor_client::solana_sdk::signature::Keypair>> =
                    Client::new(
                        Cluster::Custom(rpc_url_clone, "".to_string()),
                        Arc::new(anchor_client::solana_sdk::signature::Keypair::new()),
                    );
                let program = client.program(
                    anchor_client::solana_sdk::pubkey::Pubkey::from_str(&twine_chain_id_clone)?,
                )?;
                program
                    .rpc()
                    .get_slot_with_commitment(
                        anchor_client::solana_sdk::commitment_config::CommitmentConfig::finalized(),
                    )
                    .map_err(|e| eyre::eyre!("Failed to get current slot: {}", e))
            }
        })
        .await
        {
            Ok(Ok(slot)) => {
                break slot;
            }
            Ok(Err(e)) if retries < MAX_RETRIES => {
                retries += 1;
                error!(
                    "Failed to get current slot in poll_missing_slots (attempt {}/{}): {:?}",
                    retries, MAX_RETRIES, e
                );
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Ok(Err(e)) => return Err(e),
            Err(e) if retries < MAX_RETRIES => {
                retries += 1;
                error!(
                    "Task join error in poll_missing_slots (attempt {}/{}): {:?}",
                    retries, MAX_RETRIES, e
                );
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(e) => return Err(eyre::eyre!("Failed after retries: {}", e)),
        }
    };

    if last_synced >= current_slot {
        return Ok(Vec::new());
    }

    let mut events = Vec::new();
    let program_ids = vec![twine_chain_id, tokens_gateway_id];

    for program_id in program_ids {
        let mut retries = 0;
        let signatures = loop {
            match tokio::task::spawn_blocking({
                let rpc_url_clone = rpc_url.to_string();
                let program_id_clone = program_id.to_string();
                move || -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>, eyre::Error> {
                    let client: Client<Arc<anchor_client::solana_sdk::signature::Keypair>> =
                        Client::new(
                            Cluster::Custom(rpc_url_clone, "".to_string()),
                            Arc::new(anchor_client::solana_sdk::signature::Keypair::new()),
                        );
                    let program = client.program(anchor_client::solana_sdk::pubkey::Pubkey::from_str(
                        &program_id_clone,
                    )?)?;
                    let sigs = program
                        .rpc()
                        .get_signatures_for_address_with_config(
                            &program.id(),
                            anchor_client::solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config {
                                before: None,
                                until: None,
                                limit: Some(1000),
                                commitment: Some(
                                    anchor_client::solana_sdk::commitment_config::CommitmentConfig::finalized(),
                                ),
                            },
                        )
                        .map_err(|e| eyre::eyre!("Failed to get signatures: {}", e))?;
                    Ok(sigs)
                }
            })
            .await
            {
                Ok(Ok(sigs)) => {
                    break sigs;
                }
                Ok(Err(e)) if retries < MAX_RETRIES => {
                    retries += 1;
                    error!(
                        "Failed to get signatures for {} (attempt {}/{}): {:?}",
                        program_id, retries, MAX_RETRIES, e
                    );
                    tokio::time::sleep(RETRY_DELAY).await;
                }
                Ok(Err(e)) => return Err(e),
                Err(e) if retries < MAX_RETRIES => {
                    retries += 1;
                    error!(
                        "Task join error for signatures (attempt {}/{}): {:?}",
                        retries, MAX_RETRIES, e
                    );
                    tokio::time::sleep(RETRY_DELAY).await;
                }
                Err(e) => return Err(eyre::eyre!("Failed after retries: {}", e)),
            }
        };

        for sig_info in signatures {
            let signature =
                anchor_client::solana_sdk::signature::Signature::from_str(&sig_info.signature)?;
            let mut retries = 0;
            let tx = loop {
                match tokio::task::spawn_blocking({
                    let rpc_url_clone = rpc_url.to_string();
                    let program_id_clone = program_id.to_string();
                    move || {
                        let client: Client<Arc<anchor_client::solana_sdk::signature::Keypair>> =
                            Client::new(
                                Cluster::Custom(rpc_url_clone, "".to_string()),
                                Arc::new(anchor_client::solana_sdk::signature::Keypair::new()),
                            );
                        let program = client.program(
                            anchor_client::solana_sdk::pubkey::Pubkey::from_str(&program_id_clone)?,
                        )?;
                        program
                            .rpc()
                            .get_transaction_with_config(
                                &signature,
                                anchor_client::solana_client::rpc_config::RpcTransactionConfig {
                                    commitment: Some(
                                        anchor_client::solana_sdk::commitment_config::CommitmentConfig::finalized(),
                                    ),
                                    encoding: Some(UiTransactionEncoding::Json),
                                    max_supported_transaction_version: Some(0),
                                },
                            )
                            .map_err(|e| eyre::eyre!("Failed to get transaction: {}", e))
                    }
                })
                .await
                {
                    Ok(Ok(tx)) => {
                        break tx;
                    }
                    Ok(Err(e)) if retries < MAX_RETRIES => {
                        retries += 1;
                        error!(
                            "Failed to get transaction {} (attempt {}/{}): {:?}",
                            sig_info.signature, retries, MAX_RETRIES, e
                        );
                        tokio::time::sleep(RETRY_DELAY).await;
                    }
                    Ok(Err(e)) => return Err(e),
                    Err(e) if retries < MAX_RETRIES => {
                        retries += 1;
                        error!(
                            "Task join error for transaction {} (attempt {}/{}): {:?}",
                            sig_info.signature, retries, MAX_RETRIES, e
                        );
                        tokio::time::sleep(RETRY_DELAY).await;
                    }
                    Err(e) => return Err(eyre::eyre!("Failed after retries: {}", e)),
                }
            };

            let empty_logs = vec![];
            let logs = tx
                .transaction
                .meta
                .as_ref()
                .unwrap()
                .log_messages
                .as_ref()
                .unwrap_or(&empty_logs)
                .to_vec();

            if tx.slot > last_synced && tx.slot <= current_slot {
                events.push((logs, Some(sig_info.signature.clone())));
            }
        }
    }

    Ok(events)
}

async fn subscribe_to_single_program(
    ws_url: String,
    program_id: String,
    data_sender: Sender<(Vec<String>, Option<String>)>,
) -> Result<()> {
    info!(
        "Attempting WebSocket connection for program: {} at URL: {}",
        program_id, ws_url
    );
    let mut retries = 0;
    let (mut write, mut read) = loop {
        match connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                info!(
                    "Successfully connected to WebSocket {} for program {} after {} attempt(s)",
                    ws_url,
                    program_id,
                    retries + 1
                );
                break ws_stream.split();
            }
            Err(e) if retries < MAX_RETRIES => {
                retries += 1;
                error!(
                    "Failed to connect to WebSocket {} for program {} (attempt {}/{}): {:?}",
                    ws_url, program_id, retries, MAX_RETRIES, e
                );
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(e) => {
                error!(
                    "WebSocket connection failed after retries for program {}: {:?}",
                    program_id, e
                );
                return Err(eyre::eyre!("WebSocket connection error: {}", e));
            }
        }
    };

    info!(
        "WebSocket connected for program: {}, preparing subscription",
        program_id
    );
    let subscription = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "logsSubscribe",
        "params": [
            {
                "mentions": [program_id]
            },
            {
                "commitment": "finalized"
            }
        ]
    });

    write.send(Message::Text(subscription.to_string())).await?;
    info!("Subscription request sent for program: {}", program_id);

    if let Some(first_message) = read.next().await {
        match first_message {
            Ok(Message::Text(text)) => {
                debug!("First WebSocket response for {}: {}", program_id, text);
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                    if value.get("result").is_some() {
                        info!(
                            "Successfully subscribed to logs for program: {}",
                            program_id
                        );
                    } else if let Some(error) = value.get("error") {
                        error!("Subscription error for {}: {:?}", program_id, error);
                        return Err(eyre::eyre!("Subscription failed: {:?}", error));
                    }
                }
            }
            Ok(_) => debug!("Received non-text first message for {}", program_id),
            Err(e) => error!("First WebSocket message error for {}: {:?}", program_id, e),
        }
    } else {
        info!(
            "No first message received from WebSocket for program: {}",
            program_id
        );
    }

    info!(
        "Entering main message processing loop for program: {}",
        program_id
    );
    while let Some(message) = read.next().await {
        match message {
            Ok(Message::Text(text)) => {
                debug!("Received WebSocket message for {}: {}", program_id, text);
                let value: serde_json::Value = serde_json::from_str(&text)?;
                if let Some(params) = value.get("params") {
                    if let Some(result) = params.get("result") {
                        let signature = result
                            .get("value")
                            .and_then(|v| v.get("signature"))
                            .and_then(|s| s.as_str())
                            .map(String::from);

                        if let Some(logs) = result.get("value").and_then(|v| v.get("logs")) {
                            if let Some(logs_array) = logs.as_array() {
                                let logs_vec = logs_array
                                    .iter()
                                    .filter_map(|log| log.as_str().map(String::from))
                                    .collect::<Vec<String>>();

                                if data_sender
                                    .send((logs_vec, signature.clone()))
                                    .await
                                    .is_err()
                                {
                                    error!(
                                        "Failed to send logs through channel for {}",
                                        program_id
                                    );
                                    return Err(eyre::eyre!("Channel send error"));
                                }
                            }
                        }
                    } else if let Some(error) = value.get("error") {
                        error!("WebSocket error response for {}: {:?}", program_id, error);
                    }
                }
            }
            Ok(_) => continue,
            Err(e) => {
                error!("WebSocket error for {}: {}", program_id, e);
                return Err(eyre::eyre!("WebSocket error: {}", e));
            }
        }
    }

    Ok(())
}
