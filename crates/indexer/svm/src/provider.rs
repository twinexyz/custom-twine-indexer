use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

use futures_util::{Stream, StreamExt};
use serde_json::json;
use solana_client::{
    nonblocking::{pubsub_client::PubsubClient, rpc_client::RpcClient},
    rpc_client::GetConfirmedSignaturesForAddress2Config,
    rpc_config::{
        RpcSignaturesForAddressConfig, RpcTransactionConfig, RpcTransactionLogsConfig,
        RpcTransactionLogsFilter,
    },
    rpc_request::RpcRequest,
    rpc_response::{Response, RpcConfirmedTransactionStatusWithSignature, RpcLogsResponse},
};
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Signature};
use solana_transaction_status_client_types::{
    EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding,
};
use tokio::sync::mpsc::Receiver;
use tokio_stream::wrappers::ReceiverStream;
use tracing::error;

#[derive(Clone)]
pub struct SvmProvider {
    http: Arc<RpcClient>,
    ws: Arc<PubsubClient>,
    commitment: CommitmentConfig,
}

impl SvmProvider {
    pub async fn new(http_url: &str, ws_url: &str, chain_id: u64) -> eyre::Result<Self> {
        let pubsub_client = Arc::new(
            PubsubClient::new(ws_url)
                .await
                .map_err(|e| eyre::eyre!("Failed to connect to WebSocket: {}", e))?,
        );

        Ok(Self {
            http: Arc::new(RpcClient::new_with_timeout(
                http_url.to_string(),
                Duration::from_secs(30),
            )),
            ws: pubsub_client,

            commitment: CommitmentConfig::finalized(),
        })
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

        Ok(all_signatures)
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

    pub async fn subscribe_logs(
        &self,
        program_id: &Pubkey,
    ) -> Result<ReceiverStream<Response<RpcLogsResponse>>, eyre::Error> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let program_id = *program_id; // Copy the pubkey
        let ws_client = Arc::clone(&self.ws);

        tokio::spawn(async move {
            let filter = RpcTransactionLogsFilter::Mentions(vec![program_id.to_string()]);
            let config = RpcTransactionLogsConfig {
                commitment: Some(CommitmentConfig::finalized()),
            };

            match ws_client.logs_subscribe(filter, config).await {
                Ok((mut subscription, _unsubscriber)) => {
                    while let Some(logs) = subscription.next().await {
                        if let Err(e) = tx.send(logs).await {
                            error!("Failed to send logs: {}", e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to subscribe to logs: {}", e);
                }
            }
        });

        Ok(ReceiverStream::new(rx))
    }
}
