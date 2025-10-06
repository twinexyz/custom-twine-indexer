use std::{
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use eyre::eyre;
use futures_util::StreamExt;
use serde_json::json;
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    rpc_client::GetConfirmedSignaturesForAddress2Config,
    rpc_config::{
        RpcSignaturesForAddressConfig, RpcTransactionConfig, RpcTransactionLogsConfig,
        RpcTransactionLogsFilter,
    },
    rpc_request::RpcRequest,
    rpc_response::{Response, RpcConfirmedTransactionStatusWithSignature, RpcLogsResponse},
};
use solana_commitment_config::CommitmentConfig;
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use solana_transaction_status::option_serializer::OptionSerializer;
use solana_transaction_status_client_types::{
    EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding,
};
use tokio::sync::mpsc::Receiver;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, info};

use crate::parser::{parse_json_log, SolanaLog};

#[derive(Clone)]
pub struct SvmProvider {
    http: Arc<RpcClient>,
    commitment: CommitmentConfig,
}

impl SvmProvider {
    pub fn new(http_url: &str, chain_id: u64) -> Self {
        // let pubsub_client = Arc::new(
        //     PubsubClient::new(ws_url)
        //         .await
        //         .map_err(|e| eyre::eyre!("Failed to connect to WebSocket: {}", e))?,
        // );

        Self {
            http: Arc::new(RpcClient::new_with_timeout(
                http_url.to_string(),
                Duration::from_secs(60),
            )),

            commitment: CommitmentConfig::finalized(),
        }
    }

    pub async fn get_slot(&self) -> eyre::Result<u64> {
        self.http
            .get_slot_with_commitment(self.commitment)
            .await
            .map_err(Into::into)
    }

    pub async fn get_signature_for_address(
        &self,
        address: &Pubkey,
        start_slot: u64,
        end_slot: u64,
        batch_size: u64,
    ) -> eyre::Result<Vec<RpcConfirmedTransactionStatusWithSignature>> {
        let mut all_signatures = Vec::new();
        let mut unique_signatures: std::collections::HashMap<
            String,
            RpcConfirmedTransactionStatusWithSignature,
        > = std::collections::HashMap::new();
        let mut before: Option<String> = None;

        loop {
            let config = RpcSignaturesForAddressConfig {
                before: before.clone(),
                limit: Some(batch_size as usize),
                commitment: Some(self.commitment),
                min_context_slot: Some(start_slot),
                until: None,
            };

            let mut signatures: Vec<RpcConfirmedTransactionStatusWithSignature> = self
                .http
                .send(
                    RpcRequest::GetSignaturesForAddress,
                    json!([address.to_string(), config]),
                )
                .await?;

            let original_length = signatures.len();
            let last_signature_before_filter = signatures.last().map(|sig| sig.signature.clone());

            let mut exit_loop = false;
            signatures.retain(|sig| {
                if sig.slot > end_slot {
                    false
                } else if sig.slot < start_slot {
                    exit_loop = true;
                    false
                } else {
                    true
                }
            });

            all_signatures.extend(signatures.clone());

            // Exit conditions
            if exit_loop || original_length < batch_size as usize {
                break;
            }

            before = last_signature_before_filter;
            // before = all_signatures.last().map(|sig| sig.signature.clone());
        }

        all_signatures.sort_by(|a, b| a.slot.cmp(&b.slot).then(a.signature.cmp(&b.signature)));
        all_signatures.dedup_by(|a, b| a.signature == b.signature);

        for sig in all_signatures {
            unique_signatures
                .entry(sig.signature.clone())
                .or_insert(sig);
        }

        let mut filtered_signatures: Vec<RpcConfirmedTransactionStatusWithSignature> =
            unique_signatures.into_values().collect();

        filtered_signatures.sort_by(|a, b| a.slot.cmp(&b.slot).then(a.signature.cmp(&b.signature)));

        Ok(filtered_signatures)
    }

    pub async fn get_transaction(
        &self,
        signature: &Signature,
    ) -> eyre::Result<EncodedConfirmedTransactionWithStatusMeta> {
        let config = RpcTransactionConfig {
            commitment: Some(self.commitment),
            encoding: Some(UiTransactionEncoding::Json),
            max_supported_transaction_version: Some(0),
        };

        self.http
            .get_transaction_with_config(signature, config)
            .await
            .map_err(Into::into)
    }

    pub async fn get_logs(
        &self,
        programs: Vec<Pubkey>,
        from: u64,
        to: u64,
    ) -> eyre::Result<Vec<SolanaLog>> {
        let mut all_found_events = Vec::new();

        for program in programs {
            let signatures = self
                .get_signature_for_address(&program, from, to, 1000)
                .await?;

            debug!(
                "Found {} signatures for program {}",
                signatures.len(),
                program
            );

            if signatures.is_empty() {
                continue;
            }

            for sig in signatures {
                let signature_str = &sig.signature;
                let signature: Signature = match signature_str.parse() {
                    Ok(s) => s,
                    Err(e) => {
                        error!(
                            "Failed to parse signature string '{}': {}",
                            signature_str, e
                        );
                        continue; // Skip this signature
                    }
                };

                match self.get_transaction(&signature).await {
                    Ok(tx_with_meta) => {
                        let current_slot = tx_with_meta.slot;
                        let block_time = tx_with_meta.block_time;

                        let timestamp = block_time
                            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts as i64, 0))
                            .unwrap_or_else(|| {
                                // tracing::warn!(
                                //     "Missing or invalid block timestamp in {}. Using now.",
                                //     event_name
                                // );
                                Utc::now()
                            });

                        if let Some(meta) = tx_with_meta.transaction.meta {
                            if let OptionSerializer::Some(logs) = meta.log_messages {
                                for log in logs {
                                    if !log.starts_with("Program log:") {
                                        continue;
                                    }

                                    match parse_json_log(&log) {
                                        Ok(event) => {
                                            all_found_events.push(SolanaLog {
                                                event: event,
                                                slot_number: current_slot,
                                                signature: signature_str.clone(),
                                                timestamp: timestamp,
                                            });
                                        }
                                        Err(e) => {
                                            error!("Failed to parse log '{}': {}", log, e);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    Err(e) => {
                        error!("Erorr while fetching the transaction from signature: {}", e);
                        return Err(eyre!("Erorr while fetching the transaction from signature"));
                    }
                }
            }
        }

        all_found_events.sort_by_key(|event| event.slot_number);

        Ok(all_found_events)
    }
}
