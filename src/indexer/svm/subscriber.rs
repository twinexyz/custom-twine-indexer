use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::select;
use tokio::sync::mpsc::Sender;
use tokio::sync::Notify;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info};

#[derive(Clone)]
pub struct LogSubscriber {
    ws_url: String,
    twine_chain_id: String,
    tokens_gateway_id: String,
    data_sender: Sender<(String, Option<String>, String)>,
    shutdown: Arc<Notify>,
}

impl LogSubscriber {
    pub fn new(
        ws_url: &str,
        twine_chain_id: &str,
        tokens_gateway_id: &str,
        data_sender: Sender<(String, Option<String>, String)>,
    ) -> Self {
        Self {
            ws_url: ws_url.to_string(),
            twine_chain_id: twine_chain_id.to_string(),
            tokens_gateway_id: tokens_gateway_id.to_string(),
            data_sender,
            shutdown: Arc::new(Notify::new()),
        }
    }

    pub async fn run(&self) -> Result<()> {
        let twine_chain_task = self.spawn_subscription(&self.twine_chain_id);
        let tokens_gateway_task = self.spawn_subscription(&self.tokens_gateway_id);

        select! {
            _ = self.shutdown.notified() => {
                info!("Received shutdown signal");
                Ok(())
            }
            results = async {
                let r1 = twine_chain_task.await;
                let r2 = tokens_gateway_task.await;
                (r1, r2)
            } => {
                if let (Ok(r1), Ok(r2)) = results {
                    r1?;
                    r2?;
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Subscription tasks failed"))
                }
            }
        }
    }

    fn spawn_subscription(&self, program_id: &str) -> tokio::task::JoinHandle<Result<()>> {
        let ws_url = self.ws_url.clone();
        let program_id = program_id.to_string();
        let data_sender = self.data_sender.clone();
        let shutdown = self.shutdown.clone();

        tokio::spawn(async move {
            loop {
                select! {
                    _ = shutdown.notified() => {
                        info!("Shutting down subscription for program: {}", program_id);
                        break;
                    }
                    result = subscribe_to_single_program(&ws_url, &program_id, data_sender.clone()) => {
                        match result {
                            Ok(_) => info!("Subscription completed for {}", program_id),
                            Err(e) => {
                                error!("Subscription error for {}: {}. Reconnecting in 5s...", program_id, e);
                                sleep(Duration::from_secs(5)).await;
                                continue;
                            }
                        }
                    }
                }
            }
            Ok(())
        })
    }

    pub fn shutdown(&self) {
        self.shutdown.notify_one();
    }
}

async fn subscribe_to_single_program(
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
                debug!("Received WebSocket message for {}: {}", program_id, text);
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
                                        if log_str == "Program log: Instruction: NativeTokenDeposit" {
                                            event_type = Some("native_deposit");
                                        } else if log_str == "Program log: Instruction: SplTokensDeposit" {
                                            event_type = Some("spl_deposit");
                                        } else if log_str == "Program log: Instruction: ForcedNativeTokenWithdrawal" {
                                            event_type = Some("native_withdrawal");
                                        } else if log_str == "Program log: Instruction: ForcedSplTokenWithdrawal" {
                                            event_type = Some("spl_withdrawal");
                                        } else if log_str.starts_with("Program data: ") {
                                            encoded_data = Some(
                                                log_str
                                                    .trim_start_matches("Program data: ")
                                                    .to_string(),
                                            );
                                        }
                                    }
                                }

                                if let (Some(event_type), Some(encoded_data)) =
                                    (event_type, encoded_data.clone())
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
                                        return Err(anyhow::anyhow!("Channel send error"));
                                    }
                                } else if let Some(encoded_data) = encoded_data {
                                    let potential_event_types = if program_id
                                        == "8untebEqrCndtV6NaqhxEybS3A32sfQFXmJqmBiDYNVw"
                                    {
                                        vec![
                                            "deposit_successful",
                                            "native_withdrawal_successful",
                                            "spl_withdrawal_successful",
                                        ]
                                    } else {
                                        vec![
                                            "native_withdrawal_successful",
                                            "spl_withdrawal_successful",
                                        ]
                                    };
                                    for event_type in potential_event_types {
                                        info!(
                                            "Found encoded {} data for {}: {}",
                                            event_type, program_id, encoded_data
                                        );
                                        if data_sender
                                            .send((
                                                encoded_data.clone(),
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
                                            return Err(anyhow::anyhow!("Channel send error"));
                                        }
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
                return Err(anyhow::anyhow!("WebSocket error: {}", e));
            }
        }
    }

    Ok(())
}
