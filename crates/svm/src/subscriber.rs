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

    tokio::spawn(subscribe_to_single_program(
        ws_url_owned.clone(),
        twine_chain_id_owned,
        data_sender.clone(),
    ));
    tokio::spawn(subscribe_to_single_program(
        ws_url_owned,
        tokens_gateway_id_owned,
        data_sender,
    ));

    Ok(ReceiverStream::new(data_receiver))
}

pub async fn poll_missing_slots(
    rpc_url: &str,
    twine_chain_id: &str,
    tokens_gateway_id: &str,
    last_synced: u64,
    current_slot: u64,
    max_slots_per_request: u64,
) -> Result<Vec<(Vec<String>, Option<String>)>> {
    if last_synced >= current_slot {
        info!("Already synced to latest slot");
        return Ok(Vec::new());
    }

    info!("Max slots per request: {}", max_slots_per_request);
    info!("Current slot: {}", current_slot);

    let program_ids = vec![twine_chain_id, tokens_gateway_id];
    let mut all_events = Vec::new();
    let mut start_slot = last_synced + 1;

    while start_slot <= current_slot {
        let end_slot = (start_slot + max_slots_per_request - 1).min(current_slot);
        info!(start_slot, end_slot, "Fetching slot range");

        for program_id in &program_ids {
            let signatures = tokio::task::spawn_blocking({
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
                        .get_signatures_for_address_with_config(
                            &program.id(),
                            anchor_client::solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config {
                                before: None,
                                until: None,
                                limit: Some(max_slots_per_request as usize),
                                commitment: Some(
                                    anchor_client::solana_sdk::commitment_config::CommitmentConfig::finalized(),
                                ),
                            },
                        )
                        .map_err(|e| eyre::eyre!("Failed to get signatures: {}", e))
                }
            }).await??;

            for sig_info in signatures {
                let signature =
                    anchor_client::solana_sdk::signature::Signature::from_str(&sig_info.signature)?;
                let tx = tokio::task::spawn_blocking({
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
                .await??;

                let slot = tx.slot;
                if slot >= start_slot && slot <= end_slot {
                    let logs = tx
                        .transaction
                        .meta
                        .as_ref()
                        .unwrap()
                        .log_messages
                        .as_ref()
                        .unwrap_or(&vec![])
                        .to_vec();
                    all_events.push((logs, Some(sig_info.signature.clone())));
                }
            }
        }

        start_slot = end_slot + 1;
    }

    info!(total_events = all_events.len(), "Completed slot fetch");
    Ok(all_events)
}

async fn subscribe_to_single_program(
    ws_url: String,
    program_id: String,
    data_sender: Sender<(Vec<String>, Option<String>)>,
) -> Result<()> {
    let mut retries = 0;
    let (mut write, mut read) = loop {
        match connect_async(&ws_url).await {
            Ok((ws_stream, _)) => break ws_stream.split(),
            Err(e) if retries < MAX_RETRIES => {
                retries += 1;
                error!(
                    "Failed to connect to WebSocket (attempt {}/{}): {:?}",
                    retries, MAX_RETRIES, e
                );
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(e) => return Err(eyre::eyre!("WebSocket connection error: {}", e)),
        }
    };

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

    while let Some(message) = read.next().await {
        match message {
            Ok(Message::Text(text)) => {
                let value: serde_json::Value = serde_json::from_str(&text)?;
                if let Some(params) = value.get("params") {
                    if let Some(result) = params.get("result") {
                        let signature = result
                            .get("value")
                            .and_then(|v| v.get("signature"))
                            .and_then(|s| s.as_str())
                            .map(String::from);
                        let logs = result
                            .get("value")
                            .and_then(|v| v.get("logs"))
                            .and_then(|l| l.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|log| log.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        data_sender.send((logs, signature)).await?;
                    }
                }
            }
            Ok(_) => continue,
            Err(e) => return Err(eyre::eyre!("WebSocket error: {}", e)),
        }
    }

    Ok(())
}
