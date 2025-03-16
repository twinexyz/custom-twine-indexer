// indexer/svm/subscriber.rs
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

use solana_transaction_status_client_types::UiTransactionEncoding;

pub async fn subscribe_stream(
    ws_url: &str, // Explicitly WebSocket URL
    twine_chain_id: &str,
    tokens_gateway_id: &str,
) -> Result<impl Stream<Item = (String, Option<String>, String)>> {
    let (data_sender, data_receiver) = tokio::sync::mpsc::channel(100);

    let ws_url_owned = ws_url.to_string();
    let twine_chain_id_owned = twine_chain_id.to_string();
    let tokens_gateway_id_owned = tokens_gateway_id.to_string();

    tokio::spawn(subscribe_to_single_program(
        ws_url_owned.clone(),
        twine_chain_id_owned.clone(),
        data_sender.clone(),
        twine_chain_id_owned.clone(),
        tokens_gateway_id_owned.clone(),
    ));
    tokio::spawn(subscribe_to_single_program(
        ws_url_owned,
        tokens_gateway_id_owned.clone(),
        data_sender,
        twine_chain_id_owned,
        tokens_gateway_id_owned,
    ));

    Ok(ReceiverStream::new(data_receiver))
}

pub async fn poll_missing_slots(
    rpc_url: &str, // Explicitly HTTP RPC URL
    twine_chain_id: &str,
    tokens_gateway_id: &str,
    last_synced: u64,
) -> Result<Vec<(String, Option<String>, String)>> {
    let rpc_url_clone = rpc_url.to_string();
    let twine_chain_id_clone = twine_chain_id.to_string();

    let current_slot = tokio::task::spawn_blocking(move || {
        let client: Client<Arc<anchor_client::solana_sdk::signature::Keypair>> = Client::new(
            Cluster::Custom(rpc_url_clone, "".to_string()),
            Arc::new(anchor_client::solana_sdk::signature::Keypair::new()),
        );
        let program = client.program(anchor_client::solana_sdk::pubkey::Pubkey::from_str(
            &twine_chain_id_clone,
        )?)?;
        program
            .rpc()
            .get_slot_with_commitment(
                anchor_client::solana_sdk::commitment_config::CommitmentConfig::finalized(),
            )
            .map_err(|e| eyre::eyre!("Failed to get current slot: {}", e))
    })
    .await??;

    info!("Current slot is: {}", current_slot);
    info!("Last synced slot is: {}", last_synced);

    if last_synced >= current_slot {
        return Ok(Vec::new());
    }

    let mut events = Vec::new();
    let program_ids = vec![twine_chain_id, tokens_gateway_id];

    for program_id in program_ids {
        let rpc_url_clone = rpc_url.to_string();
        let program_id_clone = program_id.to_string();

        let signatures = tokio::task::spawn_blocking(move || -> Result<Vec<RpcConfirmedTransactionStatusWithSignature>, eyre::Error> {
            let client: Client<Arc<anchor_client::solana_sdk::signature::Keypair>> = Client::new(
                Cluster::Custom(rpc_url_clone, "".to_string()),
                Arc::new(anchor_client::solana_sdk::signature::Keypair::new()),
            );
            let program = client.program(anchor_client::solana_sdk::pubkey::Pubkey::from_str(&program_id_clone)?)?;
            let sigs = program
                .rpc()
                .get_signatures_for_address_with_config(
                    &program.id(),
                    anchor_client::solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config {
                        before: None,
                        until: None,
                        limit: None,
                        commitment: Some(anchor_client::solana_sdk::commitment_config::CommitmentConfig::finalized()),
                    },
                )
                .map_err(|e| eyre::eyre!("Failed to get signatures: {}", e))?;
            Ok(sigs)
        })
        .await??;

        info!(
            "Found {} signatures for program {}",
            signatures.len(),
            program_id
        );

        for sig_info in signatures {
            let signature =
                anchor_client::solana_sdk::signature::Signature::from_str(&sig_info.signature)?;
            let rpc_url_clone = rpc_url.to_string();
            let program_id_clone = program_id.to_string();

            let tx = tokio::task::spawn_blocking(move || {
                let client: Client<Arc<anchor_client::solana_sdk::signature::Keypair>> = Client::new(
                    Cluster::Custom(rpc_url_clone, "".to_string()),
                    Arc::new(anchor_client::solana_sdk::signature::Keypair::new()),
                );
                let program = client.program(anchor_client::solana_sdk::pubkey::Pubkey::from_str(&program_id_clone)?)?;
                program
                    .rpc()
                    .get_transaction_with_config(
                        &signature,
                        anchor_client::solana_client::rpc_config::RpcTransactionConfig {
                            commitment: Some(anchor_client::solana_sdk::commitment_config::CommitmentConfig::finalized()),
                            encoding: Some(UiTransactionEncoding::Json),
                            max_supported_transaction_version: Some(0),
                        },
                    )
                    .map_err(|e| eyre::eyre!("Failed to get transaction: {}", e))
            })
            .await??;

            let empty_logs = vec![];
            let logs = tx
                .transaction
                .meta
                .as_ref()
                .unwrap()
                .log_messages
                .as_ref()
                .unwrap_or(&empty_logs);
            debug!(
                "Transaction logs for signature {}: {:?}",
                sig_info.signature, logs
            );

            let mut event_type = None;
            let mut encoded_data = None;

            for log in logs {
                if program_id == twine_chain_id {
                    if log == "Program log: Instruction: NativeTokenDeposit" {
                        event_type = Some("native_deposit");
                    } else if log == "Program log: Instruction: SplTokensDeposit" {
                        event_type = Some("spl_deposit");
                    } else if log == "Program log: Instruction: ForcedNativeTokenWithdrawal" {
                        event_type = Some("native_withdrawal");
                    } else if log == "Program log: Instruction: ForcedSplTokenWithdrawal" {
                        event_type = Some("spl_withdrawal");
                    }
                } else if program_id == tokens_gateway_id {
                    if log == "Program log: Instruction: FinalizeNativeWithdrawal" {
                        event_type = Some("native_withdrawal_successful");
                    } else if log == "Program log: Instruction: SplWithdrawalSuccessful" {
                        event_type = Some("spl_withdrawal_successful");
                    }
                }

                if log.starts_with("Program data: ") {
                    encoded_data = Some(log.trim_start_matches("Program data: ").to_string());
                }
            }

            if let (Some(event_type), Some(encoded_data)) = (event_type, encoded_data) {
                if tx.slot > last_synced && tx.slot <= current_slot {
                    info!(
                        "Found event: type={}, slot={}, signature={}",
                        event_type, tx.slot, sig_info.signature
                    );
                    events.push((
                        encoded_data,
                        Some(sig_info.signature),
                        event_type.to_string(),
                    ));
                }
            }
        }
    }

    Ok(events)
}

async fn subscribe_to_single_program(
    ws_url: String,
    program_id: String,
    data_sender: Sender<(String, Option<String>, String)>,
    twine_chain_id: String,
    tokens_gateway_id: String,
) -> Result<()> {
    info!("Attempting to connect to WebSocket: {}", ws_url);
    let (ws_stream, _) = connect_async(&ws_url).await.map_err(|e| {
        error!("Failed to connect to WebSocket {}: {:?}", ws_url, e);
        eyre::eyre!("WebSocket connection error: {}", e)
    })?;
    info!("WebSocket connected to: {}", ws_url);
    let (mut write, mut read) = ws_stream.split();

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
    info!("Subscribed to logs for program: {}", program_id);

    if let Some(first_message) = read.next().await {
        match first_message {
            Ok(Message::Text(text)) => {
                debug!("First WebSocket response for {}: {}", program_id, text);
            }
            Ok(_) => debug!("Received non-text first message for {}", program_id),
            Err(e) => error!("First WebSocket message error for {}: {:?}", program_id, e),
        }
    }

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
                                let mut event_type = None;
                                let mut encoded_data = None;

                                for log in logs_array {
                                    if let Some(log_str) = log.as_str() {
                                        if program_id == twine_chain_id {
                                            if log_str == "Program log: Instruction: NativeTokenDeposit" {
                                                event_type = Some("native_deposit");
                                            } else if log_str == "Program log: Instruction: SplTokensDeposit" {
                                                event_type = Some("spl_deposit");
                                            } else if log_str == "Program log: Instruction: ForcedNativeTokenWithdrawal" {
                                                event_type = Some("native_withdrawal");
                                            } else if log_str == "Program log: Instruction: ForcedSplTokenWithdrawal" {
                                                event_type = Some("spl_withdrawal");
                                            }
                                        } else if program_id == tokens_gateway_id {
                                            if log_str == "Program log: Instruction: FinalizeNativeWithdrawal" {
                                                event_type = Some("finalize_native_withdrawal");
                                            } else if log_str == "Program log: Instruction: FinalizeSplWithdrawal" {
                                                event_type = Some("finalize_spl_withdrawal");
                                            }
                                        }

                                        if log_str.starts_with("Program data: ") {
                                            encoded_data = Some(
                                                log_str
                                                    .trim_start_matches("Program data: ")
                                                    .to_string(),
                                            );
                                        }
                                    }
                                }

                                if let (Some(event_type), Some(encoded_data)) =
                                    (event_type, encoded_data)
                                {
                                    info!(
                                        "Found encoded {} data for {}: {}",
                                        event_type, program_id, encoded_data
                                    );
                                    if data_sender
                                        .send((
                                            encoded_data,
                                            signature.clone(),
                                            event_type.to_string(),
                                        ))
                                        .await
                                        .is_err()
                                    {
                                        error!(
                                            "Failed to send {} data through channel for {}",
                                            event_type, program_id
                                        );
                                        return Err(eyre::eyre!("Channel send error"));
                                    }
                                }
                            }
                        }
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

    info!("WebSocket stream for {} ended", program_id);
    Ok(())
}
