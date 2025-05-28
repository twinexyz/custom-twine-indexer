use std::{sync::Arc, time::Duration};

use common::config::ChainConfig;
use database::client::DbClient;
use eyre::Error;
use futures_util::{stream::select_all, StreamExt};
use solana_sdk::pubkey::Pubkey;
use solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta;
use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{error, info, instrument};

use crate::{handler::SolanaEventHandler, parser::FoundEvent, provider::SvmProvider};

pub struct SolanaIndexer {
    provider: SvmProvider,
    handler: SolanaEventHandler,
    max_batch_size: usize,
    db_client: Arc<DbClient>,
    config: ChainConfig,
    program_ids: Vec<Pubkey>,
}

type Log = (Vec<String>, Option<String>);

impl SolanaIndexer {
    pub async fn new(db: Arc<DbClient>, handler: SolanaEventHandler) -> eyre::Result<Self> {
        let config = handler.get_chain_config();

        let provider =
            SvmProvider::new(&config.http_rpc_url, &config.ws_rpc_url, config.chain_id).await?;

        Ok(Self {
            provider,
            handler,
            max_batch_size: config.block_sync_batch_size as usize,
            db_client: db,
            config,
            program_ids: Vec::new(),
        })
    }

    async fn initialize_state(&self) -> Result<u64, Error> {
        let last_synced = self
            .db_client
            .get_last_synced_height(self.handler.chain_id() as i64, self.config.start_block)
            .await
            .unwrap_or(self.config.start_block as i64);
        Ok(last_synced as u64)
    }

    pub async fn run(&mut self) -> eyre::Result<()> {
        let initial_height = self.initialize_state().await?;

        match self.sync_historical(initial_height).await {
            Ok(()) => {
                info!("Historical sync completed successfully");
                // self.sync_live().await?;
            }
            Err(e) => {}
        }

        Ok(())
    }

    #[instrument(skip_all, fields(CHAIN = "Solana"))]
    async fn sync_historical(&self, from: u64) -> eyre::Result<()> {
        let current_height = self.provider.get_slot().await?;
        let mut start_height = from;

        let mut retries = 0;

        if start_height >= current_height {
            info!(
                "Historical sync caught up to block {}. Switching to live or sleeping.",
                start_height
            );

            ()
        }

        while start_height < current_height {
            let end_height = (start_height + self.config.block_sync_batch_size).min(current_height);

            info!(
                "Missing heights: {} and Fetching logs from height {} to {}",
                current_height - start_height,
                start_height,
                end_height
            );

            match self
                .provider
                .get_logs(
                    self.handler.get_program_addresses(),
                    start_height,
                    end_height,
                )
                .await
            {
                Ok(logs) => {
                    if logs.is_empty() {
                        info!(
                            "No relevant logs found in heights from {} to {}",
                            start_height, end_height
                        );
                    } else {
                        info!(
                            "Processing {} logs from blocks {} to {}",
                            logs.len(),
                            start_height,
                            end_height
                        );
                        match self.process_logs(logs, end_height).await {
                            Ok(_) => {
                                info!(
                                    "Successfully processed and persisted logs for blocks {} to {}",
                                    start_height, end_height
                                );
                            }
                            Err(e) => {
                                error!("Error processing batch for blocks {} to {}: {:?}. Will retry this batch.", start_height, end_height, e);
                                // Implement retry logic with backoff, or halt if unrecoverable
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                continue; // Retry the same batch
                            }
                        }
                    }

                    start_height = end_height + 1;
                }

                Err(e) => {
                    retries += 1;

                    error!(
                        "Error while getting logs from {:?} to {:?}",
                        start_height, end_height
                    );

                    let delay = Duration::from_secs(5);

                    tokio::time::sleep(delay * 2u32.pow(retries)).await;
                    continue; // Retry the same batch
                }
            }
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

            // self.handler.handle_event(&logs, Some(sig)).await;
        });
    }

    async fn process_logs(&self, logs: Vec<FoundEvent>, max_seen_height: u64) -> eyre::Result<()> {
        let concurrency_limit = 1000;
        let semaphore = Arc::new(Semaphore::new(concurrency_limit));
        let handler = self.handler.clone();

        let mut prepare_tasks = JoinSet::new();

        for log in logs {
            let permit = semaphore.clone().acquire_owned().await?;
            let handler = handler.clone();

            let log_clone = log.clone();
            prepare_tasks.spawn(async move {
                let result = handler.handle_event(log_clone).await; // Returns Result<EventDataEnum, ParserError>
                drop(permit);

                result
            });
        }

        let mut prepared_event_data_results = Vec::new();
        let mut batch_had_errors = false;
        while let Some(task_result) = prepare_tasks.join_next().await {
            match task_result {
                Ok(Ok(task)) => prepared_event_data_results.push(task),

                Ok(Err(parser_error)) => {
                    // Log individual parser error, decide if it's critical

                    batch_had_errors = true;
                    error!(
                        "Event parsing failed: {:?}. Storing as error.",
                        parser_error
                    );
                }

                Err(join_error) => {
                    // A task panicked. This is more severe.
                    error!(
                        "Task panicked during event preparation: {:?}. Aborting batch.",
                        join_error
                    );
                    return Err(eyre::eyre!(
                        "Event preparation task panicked: {}",
                        join_error
                    ));
                }
            }
        }

        if batch_had_errors {
            return Err(eyre::eyre!(
                "One or more events failed parsing in the batch. See logs for details."
            ));
        }

        if prepared_event_data_results.is_empty() {
            info!("No successfully prepared event data to process ");
            return Ok(());
        }

        info!(
            "Successfully prepared events: {}",
            prepared_event_data_results.len()
        );

        match self
            .db_client
            .process_bulk_l1_database_operations(prepared_event_data_results)
            .await
        {
            Ok(_) => {
                info!("Succesfully updated the database");
                self.db_client
                    .upsert_last_synced(self.handler.chain_id() as i64, max_seen_height as i64)
                    .await?;
            }

            Err(e) => {
                return Err(eyre::eyre!(
                    "Error while bulking database operation {:?}",
                    e
                ));
            }
        }

        Ok(())
    }
}
