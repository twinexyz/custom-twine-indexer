use std::sync::Arc;

use common::config::ChainConfig;
use database::client::DbClient;
use futures_util::{stream::select_all, StreamExt};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta;

use crate::{handler::SolanaEventHandler, provider::SvmProvider};

pub struct SolanaIndexer {
    provider: SvmProvider,
    handler: SolanaEventHandler,
    max_batch_size: usize,
    db_client: Arc<DbClient>,
    config: ChainConfig,
    program_ids: Vec<Pubkey>,
}

impl SolanaIndexer {
    pub async fn new(config: ChainConfig, db: Arc<DbClient>, handler: SolanaEventHandler) -> Self {
        let provider =
            SvmProvider::new(&config.http_rpc_url, &config.ws_rpc_url, config.chain_id).await;

        Self {
            provider,
            handler,
            max_batch_size: config.block_sync_batch_size as usize,
            db_client: db,
            config,
            program_ids: Vec::new(),
        }
    }

    pub async fn run(&mut self) -> eyre::Result<()> {
        let current_slot = self.provider.get_slot().await?;
        let last_synced = self
            .db_client
            .get_last_synced_height(self.config.chain_id as i64, self.config.start_block);

        Ok(())
    }

    async fn catchup_historical(&self, from: u64, to: u64) -> eyre::Result<()> {
        let mut current_slot = from;

        while current_slot <= to {
            let end_slot = (current_slot + self.max_batch_size as u64).min(to);

            for program_id in &self.program_ids {
                let signatures = self
                    .provider
                    .get_signature_for_address(program_id, current_slot, end_slot, 1000)
                    .await?;

                for sig in signatures {
                    let tx = self
                        .provider
                        .get_transaction(&sig.signature.parse()?)
                        .await?;
                    let logs = extract_logs_from_transaction(&tx);

                    self.handler.handle_event(&logs, Some(sig.signature)).await;
                }
            }

            current_slot = end_slot + 1;
        }
        Ok(())
    }

    async fn stream_live(&self) {
        let mut streams = vec![];

        for program_id in &self.program_ids {
            let stream = self.provider.subscribe_logs(program_id).await.unwrap();
            streams.push(stream);
        }

        let merged = select_all(streams);

        merged.for_each(|result| async {
            let value = result.value;
            let logs = value.logs;
            let sig = value.signature;

            self.handler.handle_event(&logs, Some(sig)).await
        });
    }
}

fn extract_logs_from_transaction(tx: &EncodedConfirmedTransactionWithStatusMeta) -> Vec<String> {
    let logs = tx.transaction.meta.as_ref().unwrap();

    logs.log_messages.clone().unwrap()
}
