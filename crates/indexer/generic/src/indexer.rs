use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use eyre::Error;
use tokio::{sync::Semaphore, task::JoinSet};
use tracing::{error, info, instrument};

use crate::handler::ChainEventHandler;

#[async_trait]
pub trait ChainIndexer: Send + Sync {
    type EventHandler: ChainEventHandler + Clone + Send + Sync + 'static;

    // async fn sync_historical(&self, from: u64) -> eyre::Result<()>;
    async fn sync_live(&self) -> eyre::Result<()>;
    async fn get_initial_state(&self) -> eyre::Result<u64>;

    async fn get_historical_logs(
        &self,
        from: u64,
        to: u64,
    ) -> eyre::Result<Vec<<Self::EventHandler as ChainEventHandler>::LogType>>;

    async fn get_current_chain_height(&self) -> eyre::Result<u64>;

    fn get_event_handler(&self) -> Self::EventHandler;

    async fn run(&mut self) -> Result<(), Error> {
        let initial_height = self.get_initial_state().await?;

        match self.sync_historical(initial_height).await {
            Ok(()) => {
                info!("Historical sync completed successfully");
                self.sync_live().await?;
            }
            Err(e) => {
                error!("Error during historical sync: {:?}", e);
                return Err(e);
            }
        }

        Ok(())
    }

    #[instrument(skip_all, fields(CHAIN = %self.get_event_handler().chain_id()))]
    async fn sync_historical(&self, from: u64) -> Result<(), Error> {
        let current_block = self.get_current_chain_height().await?;
        let mut start_block = from;

        let mut reconnect_attempt = 0;

        if start_block >= current_block {
            info!(
                "Historical sync caught up to block {}. Switching to live or sleeping.",
                start_block
            );

            ()
        }

        while start_block < current_block {
            let end_block = (start_block
                + self
                    .get_event_handler()
                    .get_chain_config()
                    .block_sync_batch_size)
                .min(current_block);

            info!(
                "Missing blocks: {} and Fetching logs for blocks {} to {}",
                current_block - start_block,
                start_block,
                end_block
            );

            match self.get_historical_logs(start_block, end_block).await {
                Ok(logs) => {
                    if logs.is_empty() {
                        info!(
                            "No relevant logs found in blocks {} to {}",
                            start_block, end_block
                        );
                    } else {
                        info!(
                            "Processing {} logs from blocks {} to {}",
                            logs.len(),
                            start_block,
                            end_block
                        );

                        match self.process_logs(logs, end_block).await {
                            Ok(_) => {
                                info!(
                                    "Successfully processed and persisted logs for blocks {} to {}",
                                    start_block, end_block
                                );
                            }
                            Err(e) => {
                                error!(
                                    "Error processing batch for blocks {} to {}: {:?}. Will retry this batch.",
                                    start_block, end_block, e
                                );
                                // Implement retry logic with backoff, or halt if unrecoverable
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                continue; // Retry the same batch
                            }
                        }
                    }

                    start_block = end_block + 1;
                }

                Err(e) => {
                    reconnect_attempt += 1;

                    error!(
                        "Error while getting logs from {:?} to {:?}, error: {:?}",
                        start_block, end_block, e
                    );

                    let delay = Duration::from_secs(5);

                    tokio::time::sleep(delay * 2u32.pow(reconnect_attempt)).await;
                    continue; // Retry the same batch
                }
            }
        }
        Ok(())
    }

    async fn process_logs(
        &self,
        logs: Vec<<Self::EventHandler as ChainEventHandler>::LogType>,
        max_seen_height: u64,
    ) -> eyre::Result<()> {
        let concurrency_limit = 1000;
        let semaphore = Arc::new(Semaphore::new(concurrency_limit));
        let handler = self.get_event_handler();

        let mut prepare_tasks = JoinSet::new();

        for log in logs {
            let permit = semaphore.clone().acquire_owned().await?;
            let handler_clone = handler.clone();

            let log_clone = log.clone();
            prepare_tasks.spawn(async move {
                let result = handler_clone.handle_event(log_clone).await;
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

        match handler
            .get_db_client()
            .process_bulk_l1_database_operations(prepared_event_data_results)
            .await
        {
            Ok(_) => {
                info!("Succesfully updated the database");
                handler
                    .get_db_client()
                    .upsert_last_synced(handler.chain_id() as i64, max_seen_height as i64)
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
    async fn process_buffer(
        &self,
        buffer: &mut Vec<<Self::EventHandler as ChainEventHandler>::LogType>,
        max_seen_block: i64,
    ) -> Result<(), Error> {
        let logs = std::mem::replace(buffer, Vec::with_capacity(100));
        self.process_logs(logs, max_seen_block as u64).await?;
        Ok(())
    }
}
