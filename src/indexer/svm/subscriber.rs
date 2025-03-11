use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info};

pub async fn subscribe_to_logs(
    ws_url: &str,
    program_id: &str,
    data_sender: Sender<(String, Option<String>, String)>,
) -> Result<()> {
    let (ws_stream, _) = connect_async(ws_url)
        .await
        .context("Failed to connect to WebSocket")?;
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

    write
        .send(Message::Text(subscription.to_string()))
        .await
        .context("Failed to send subscription request")?;
    info!("Subscribed to logs for program: {}", program_id);

    while let Some(message) = read.next().await {
        match message {
            Ok(Message::Text(text)) => {
                debug!("Received WebSocket message: {}", text);
                let value: Value =
                    serde_json::from_str(&text).context("Failed to parse WebSocket message")?;

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
                                        // Detect the event type based on the instruction log
                                        if log_str == "Program log: Instruction: NativeTokenDeposit" {
                                            event_type = Some("native_deposit");
                                        } else if log_str == "Program log: Instruction: SplTokensDeposit" {
                                            event_type = Some("spl_deposit");
                                        } else if log_str == "Program log: Instruction: ForcedNativeTokenWithdrawal" {
                                            event_type = Some("native_withdrawal");
                                        } else if log_str == "Program log: Instruction: ForcedSplTokenWithdrawal" {
                                            event_type = Some("spl_withdrawal");
                                        } else if log_str.starts_with("Program data: ") {
                                            encoded_data = Some(log_str.trim_start_matches("Program data: ").to_string());
                                        }
                                    }
                                }

                                if let (Some(event_type), Some(encoded_data)) =
                                    (event_type, encoded_data)
                                {
                                    info!("Found encoded {} data: {}", event_type, encoded_data);
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
                                            "Failed to send {} data through channel",
                                            event_type
                                        );
                                        return Err(anyhow::anyhow!("Channel send error"));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Ok(_) => continue,
            Err(e) => {
                error!("WebSocket error: {}", e);
                return Err(anyhow::anyhow!("WebSocket error: {}", e));
            }
        }
    }

    Ok(())
}
