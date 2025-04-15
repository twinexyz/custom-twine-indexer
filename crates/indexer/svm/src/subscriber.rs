use super::{MAX_RETRIES, RETRY_DELAY};
use anchor_client::{
    solana_client::rpc_response::{Response, RpcConfirmedTransactionStatusWithSignature},
    solana_sdk::transaction::TransactionError,
    Client, Cluster,
};
use eyre::{eyre, Result};
use futures_util::{SinkExt, Stream, StreamExt};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use solana_transaction_status_client_types::{
    EncodedConfirmedTransactionWithStatusMeta, TransactionConfirmationStatus, UiTransactionEncoding,
};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info, warn};

// Struct for RPC signature response (for reqwest)
#[derive(Serialize, Deserialize, Debug, Clone)]
struct RpcSignatureInfo {
    signature: String,
    slot: u64,
    err: Option<Value>,
    memo: Option<String>,
    #[serde(rename = "blockTime")]
    block_time: Option<i64>,
    #[serde(rename = "confirmationStatus")]
    confirmation_status: Option<String>,
}

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

pub async fn poll_missing_slots(
    rpc_url: &str,
    twine_chain_id: &str,
    tokens_gateway_id: &str,
    last_synced: u64,
    current_slot: u64,
    max_slots_per_request: u64,
) -> Result<Vec<(Vec<String>, Option<String>)>> {
    if last_synced >= current_slot {
        debug!("Already synced to latest slot {}", current_slot);
        return Ok(Vec::new());
    }

    info!(
        "Starting historical sync from slot {} to {} (max slots per request: {})",
        last_synced + 1,
        current_slot,
        max_slots_per_request
    );

    let program_ids = vec![twine_chain_id, tokens_gateway_id];
    let mut all_events = Vec::new();
    let mut start_slot = last_synced + 1;
    let total_slots = current_slot - last_synced;

    while start_slot <= current_slot {
        let end_slot = (start_slot + max_slots_per_request - 1).min(current_slot);
        info!(
            "Fetching transactions for slots {}-{}",
            start_slot, end_slot
        );

        // Fetch signatures for the current slot range
        let mut slot_signatures = Vec::new();
        for program_id in &program_ids {
            let signatures =
                fetch_signatures_for_program(rpc_url, program_id, start_slot, end_slot).await?;
            debug!(
                "Fetched {} signatures for program {} in slots {}-{}",
                signatures.len(),
                program_id,
                start_slot,
                end_slot
            );
            slot_signatures.extend(signatures);
        }

        // Sort and deduplicate signatures by slot and signature
        slot_signatures.sort_by(|a, b| a.slot.cmp(&b.slot).then(a.signature.cmp(&b.signature)));
        slot_signatures.dedup_by(|a, b| a.signature == b.signature);

        debug!(
            "Total {} unique signatures for slots {}-{}",
            slot_signatures.len(),
            start_slot,
            end_slot
        );

        // Fetch transaction details for each signature
        for sig_info in slot_signatures {
            let signature =
                anchor_client::solana_sdk::signature::Signature::from_str(&sig_info.signature)?;
            let tx = match fetch_transaction(rpc_url, &signature).await {
                Ok(tx) => tx,
                Err(e) => {
                    error!("Failed to fetch transaction {}: {}", sig_info.signature, e);
                    continue;
                }
            };

            let slot = tx.slot;
            if slot >= start_slot && slot <= end_slot {
                let logs = tx
                    .transaction
                    .meta
                    .as_ref()
                    .and_then(|meta| {
                        let log_messages: Option<Vec<String>> = meta.log_messages.clone().into();
                        log_messages
                    })
                    .unwrap_or_default();
                debug!(
                    "Transaction {} in slot {} has {} logs",
                    sig_info.signature,
                    slot,
                    logs.len()
                );
                if !logs.is_empty() {
                    all_events.push((logs, Some(sig_info.signature)));
                }
            }
        }

        start_slot = end_slot + 1;
    }

    info!(
        "Completed historical sync: fetched {} events across {} slots",
        all_events.len(),
        total_slots
    );
    Ok(all_events)
}

async fn fetch_signatures_for_program(
    rpc_url: &str,
    program_id: &str,
    start_slot: u64,
    end_slot: u64,
) -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>> {
    let mut retries = 0;
    loop {
        match try_fetch_signatures(rpc_url, program_id, start_slot, end_slot).await {
            Ok(signatures) => {
                if signatures.is_empty() {
                    warn!(
                        "No signatures found for program {} in slots {}-{}",
                        program_id, start_slot, end_slot
                    );
                } else {
                    info!(
                        "Fetched {} signatures for program {} in slots {}-{}",
                        signatures.len(),
                        program_id,
                        start_slot,
                        end_slot
                    );
                }
                return Ok(signatures);
            }
            Err(e) if retries < MAX_RETRIES => {
                retries += 1;
                error!(
                    "Failed to fetch signatures for program {} in slots {}-{} (attempt {}/{}): {}. Retrying after {}ms...",
                    program_id, start_slot, end_slot, retries, MAX_RETRIES, e, RETRY_DELAY.as_millis()
                );
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(e) => {
                error!(
                    "Exhausted retries fetching signatures for program {} in slots {}-{}: {}",
                    program_id, start_slot, end_slot, e
                );
                return Err(e);
            }
        }
    }
}

async fn try_fetch_signatures(
    rpc_url: &str,
    program_id: &str,
    start_slot: u64,
    end_slot: u64,
) -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>> {
    let client = HttpClient::new();
    let mut all_signatures = Vec::new();
    let mut before_signature: Option<String> = None;

    loop {
        let request = json!({
            "jsonrpc": "2.0",
            "id": all_signatures.len() + 1,
            "method": "getSignaturesForAddress",
            "params": [
                program_id,
                {
                    "limit": 1000,
                    "commitment": "finalized",
                    "minContextSlot": start_slot,
                    "before": before_signature
                }
            ]
        });

        debug!(
            "Sending getSignaturesForAddress for program {}: {}",
            program_id, request
        );

        let response = client
            .post(rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| eyre!("HTTP request failed: {}", e))?
            .json::<Value>()
            .await
            .map_err(|e| eyre!("Failed to parse response: {}", e))?;

        debug!("Received response: {:?}", response);

        if let Some(error) = response.get("error") {
            return Err(eyre!("RPC error: {}", error));
        }

        let signatures: Vec<RpcSignatureInfo> = response
            .get("result")
            .and_then(|result| serde_json::from_value(result.clone()).ok())
            .unwrap_or_default();

        debug!(
            "Parsed {} signatures for program {}: {:?}",
            signatures.len(),
            program_id,
            signatures
                .iter()
                .map(|s| (s.signature.clone(), s.slot))
                .collect::<Vec<_>>()
        );

        let filtered: Vec<_> = signatures
            .into_iter()
            .filter(|sig| {
                let in_range = sig.slot >= start_slot && sig.slot <= end_slot;
                if !in_range {
                    debug!(
                        "Excluding signature {} in slot {} (outside range {}-{})",
                        sig.signature, sig.slot, start_slot, end_slot
                    );
                } else if sig.slot == 373475385 {
                    info!("Found signature {} in target slot 373475385", sig.signature);
                }
                in_range
            })
            .map(|sig| RpcConfirmedTransactionStatusWithSignature {
                signature: sig.signature,
                slot: sig.slot,
                err: sig.err.and_then(|err| serde_json::from_value(err).ok()),
                memo: sig.memo,
                block_time: sig.block_time,
                confirmation_status: sig.confirmation_status.and_then(|status| {
                    match status.as_str() {
                        "processed" => Some(TransactionConfirmationStatus::Processed),
                        "confirmed" => Some(TransactionConfirmationStatus::Confirmed),
                        "finalized" => Some(TransactionConfirmationStatus::Finalized),
                        _ => None,
                    }
                }),
            })
            .collect();

        all_signatures.extend(filtered.clone());

        if filtered.len() < 1000 {
            break;
        }

        before_signature = filtered.last().map(|sig| sig.signature.clone());
    }

    debug!(
        "Total {} signatures for program {} in slots {}-{}: {:?}",
        all_signatures.len(),
        program_id,
        start_slot,
        end_slot,
        all_signatures
            .iter()
            .map(|s| s.signature.clone())
            .collect::<Vec<_>>()
    );

    Ok(all_signatures)
}

async fn fetch_transaction(
    rpc_url: &str,
    signature: &anchor_client::solana_sdk::signature::Signature,
) -> Result<EncodedConfirmedTransactionWithStatusMeta> {
    let mut retries = 0;
    loop {
        match try_fetch_transaction(rpc_url, signature).await {
            Ok(tx) => return Ok(tx),
            Err(e) if retries < MAX_RETRIES => {
                retries += 1;
                error!(
                    "Failed to fetch transaction {} (attempt {}/{}): {}. Retrying after {}ms...",
                    signature,
                    retries,
                    MAX_RETRIES,
                    e,
                    RETRY_DELAY.as_millis()
                );
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(e) => {
                error!(
                    "Exhausted retries fetching transaction {}: {}",
                    signature, e
                );
                return Err(e);
            }
        }
    }
}

async fn try_fetch_transaction(
    rpc_url: &str,
    signature: &anchor_client::solana_sdk::signature::Signature,
) -> Result<EncodedConfirmedTransactionWithStatusMeta> {
    tokio::task::spawn_blocking({
        let rpc_url = rpc_url.to_string();
        let signature = *signature;
        move || -> Result<EncodedConfirmedTransactionWithStatusMeta> {
            let client: Client<Arc<anchor_client::solana_sdk::signature::Keypair>> =
                Client::new(
                    Cluster::Custom(rpc_url, "".to_string()),
                    Arc::new(anchor_client::solana_sdk::signature::Keypair::new()),
                );
            let program = client.program(anchor_client::solana_sdk::pubkey::Pubkey::new_unique())?;
            let tx = program
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
                .map_err(|e| eyre!("Failed to get transaction {}: {}", signature, e))?;
            if tx.transaction.meta.is_none() {
                return Err(eyre!("Transaction {} not found or has no metadata", signature));
            }
            Ok(tx)
        }
    })
    .await?
}

async fn subscribe_to_single_program(
    ws_url: String,
    program_id: String,
    data_sender: Sender<(Vec<String>, Option<String>)>,
) -> Result<()> {
    let mut retries = 0;
    loop {
        match try_subscribe(&ws_url, &program_id, &data_sender).await {
            Ok(_) => {
                info!("Subscription ended normally for program {}", program_id);
                break Ok(());
            }
            Err(e) if retries < MAX_RETRIES => {
                retries += 1;
                error!(
                    "Failed to subscribe to program {} (attempt {}/{}): {}. Retrying after {}ms...",
                    program_id,
                    retries,
                    MAX_RETRIES,
                    e,
                    RETRY_DELAY.as_millis()
                );
                tokio::time::sleep(RETRY_DELAY).await;
            }
            Err(e) => {
                error!(
                    "Fatal error subscribing to program {} after {} retries: {}",
                    program_id, MAX_RETRIES, e
                );
                return Err(e);
            }
        }
    }
}

async fn try_subscribe(
    ws_url: &str,
    program_id: &str,
    data_sender: &Sender<(Vec<String>, Option<String>)>,
) -> Result<()> {
    let (mut write, mut read) = connect_async(ws_url).await?.0.split();
    let subscription = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "logsSubscribe",
        "params": [
            {"mentions": [program_id]},
            {"commitment": "finalized"}
        ]
    });
    write.send(Message::Text(subscription.to_string())).await?;
    info!("Subscription request sent for program {}", program_id);

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

                        if !logs.is_empty() {
                            info!(
                                "Received event with {} logs for program {}, signature: {:?}",
                                logs.len(),
                                program_id,
                                signature
                            );
                            debug!("Event logs: {:?}", logs);
                            if let Err(e) = data_sender.send((logs, signature)).await {
                                error!("Failed to send event for program {}: {}", program_id, e);
                            }
                        }
                    }
                }
            }
            Ok(_) => {
                debug!("Received non-text message for program {}", program_id);
            }
            Err(e) => {
                error!("WebSocket error for program {}: {}", program_id, e);
                return Err(eyre!("WebSocket error for program {}: {}", program_id, e));
            }
        }
    }
    Ok(())
}
