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
        "Starting subscription for twine_chain_id: {}",
        twine_chain_id_owned
    );
    tokio::spawn(subscribe_to_single_program(
        ws_url_owned.clone(),
        twine_chain_id_owned,
        data_sender.clone(),
    ));

    info!(
        "Starting subscription for tokens_gateway_id: {}",
        tokens_gateway_id_owned
    );
    tokio::spawn(subscribe_to_single_program(
        ws_url_owned,
        tokens_gateway_id_owned,
        data_sender,
    ));

    info!("Subscription stream initialized, waiting for events...");
    Ok(ReceiverStream::new(data_receiver))
}

async fn subscribe_to_single_program(
    ws_url: String,
    program_id: String,
    data_sender: Sender<(Vec<String>, Option<String>)>,
) -> Result<()> {
    loop {
        let mut retries = 0;
        let (mut write, mut read) = loop {
            info!(
                "Attempting WebSocket connection to {} for program {}",
                ws_url, program_id
            );
            match connect_async(&ws_url).await {
                Ok((ws_stream, _)) => {
                    info!(
                        "Successfully connected to WebSocket for program {}",
                        program_id
                    );
                    break ws_stream.split();
                }
                Err(e) if retries < MAX_RETRIES => {
                    retries += 1;
                    error!(
                        "Failed to connect to WebSocket for program {} (attempt {}/{}): {:?}",
                        program_id, retries, MAX_RETRIES, e
                    );
                    tokio::time::sleep(RETRY_DELAY).await;
                }
                Err(e) => {
                    error!(
                        "Fatal WebSocket connection error for program {} after {} retries: {}",
                        program_id, MAX_RETRIES, e
                    );
                    tokio::time::sleep(RETRY_DELAY).await;
                    continue;
                }
            }
        };

        // Send subscription request
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

        if let Err(e) = write.send(Message::Text(subscription.to_string())).await {
            error!(
                "Failed to send subscription request for program {}: {}",
                program_id, e
            );
            tokio::time::sleep(RETRY_DELAY).await;
            continue;
        }
        info!(
            "Subscription request sent successfully for program {}",
            program_id
        );

        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    debug!(
                        "Received WebSocket message for program {}: {}",
                        program_id, text
                    );
                    let value: serde_json::Value = match serde_json::from_str(&text) {
                        Ok(val) => val,
                        Err(e) => {
                            error!("Failed to parse JSON for program {}: {}", program_id, e);
                            continue;
                        }
                    };

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
                                        .collect::<Vec<String>>()
                                })
                                .unwrap_or_default();

                            if logs.is_empty() {
                                info!("Received event with no logs for program {}", program_id);
                            } else {
                                info!(
                                    "Received event with {} logs for program {}, signature: {:?}",
                                    logs.len(),
                                    program_id,
                                    signature
                                );
                                debug!("Event logs: {:?}", logs);
                            }

                            match data_sender.send((logs, signature.clone())).await {
                                Ok(_) => info!(
                                    "Successfully sent event to channel for program {}, signature: {:?}",
                                    program_id,
                                    signature
                                ),
                                Err(e) => error!(
                                    "Failed to send event to channel for program {}: {}",
                                    program_id, e
                                ),
                            }
                        } else {
                            debug!("No 'result' field in params for program {}", program_id);
                        }
                    } else {
                        debug!("No 'params' field in message for program {}", program_id);
                    }
                }
                Ok(_) => {
                    debug!("Received non-text message for program {}", program_id);
                    continue;
                }
                Err(e) => {
                    error!("WebSocket error for program {}: {}", program_id, e);
                    tokio::time::sleep(RETRY_DELAY).await;
                    break;
                }
            }
        }

        info!(
            "WebSocket connection lost for program {}, attempting to reconnect...",
            program_id
        );
        tokio::time::sleep(RETRY_DELAY).await;
    }
}
