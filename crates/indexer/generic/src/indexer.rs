use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{handler::ChainEventHandler, state::IndexerState};
use async_trait::async_trait;
use common::config::IndexerSettings;
use database::client::DbClient;
use eyre::Error;
use tokio::{sync::Semaphore, task::JoinSet, time::sleep};
use tracing::{debug, error, info, instrument};

const MAX_RETRIES: i32 = 20;

#[async_trait]
pub trait ChainIndexer: Send + Sync {
    type EventHandler: ChainEventHandler + Clone + Send + Sync + 'static;

    async fn get_initial_state(&self) -> eyre::Result<u64>;

    async fn get_historical_logs(
        &self,
        from: u64,
        to: u64,
    ) -> eyre::Result<Vec<<Self::EventHandler as ChainEventHandler>::LogType>>;

    async fn get_current_chain_height(&self) -> eyre::Result<u64>;
    fn get_block_number_from_log(
        &self,
        log: &<Self::EventHandler as ChainEventHandler>::LogType,
    ) -> Option<u64>;

    fn get_event_handler(&self) -> Self::EventHandler;
    fn get_indexer_settings(&self) -> IndexerSettings;

    fn get_db_client(&self) -> Arc<DbClient>;

    #[instrument(skip_all, fields(CHAIN = %self.get_event_handler().chain_id()))]
    async fn run(&mut self) -> Result<(), Error> {
        let initial_height = self.get_initial_state().await?;

        info!(
            "Initial height for chain: {} is {}",
            initial_height,
            self.get_event_handler().chain_id()
        );

        let mut indexer_state = IndexerState::new(
            initial_height,
            self.get_event_handler().chain_id(),
            self.get_event_handler().get_chain_config().block_time_ms,
        );

        match self.sync_chain(&mut indexer_state).await {
            Ok(()) => {
                info!("Historical sync completed successfully");
            }
            Err(e) => {
                error!("Error during historical sync: {:?}", e);
                return Err(e);
            }
        }

        Ok(())
    }

    #[instrument(skip_all, fields(CHAIN = %self.get_event_handler().chain_id()))]
    async fn sync_chain(&self, indexer_state: &mut IndexerState) -> Result<(), Error> {
        let chain_config = self.get_event_handler().get_chain_config();
        let block_time_ms = chain_config.block_time_ms;
        let batch_size = chain_config.block_sync_batch_size;
        loop {
            let current_chain_height = match self.get_current_chain_height().await {
                Ok(height) => height,
                Err(e) => {
                    error!("Error while getting current chain height: {:?}", e);
                    sleep(Duration::from_secs(block_time_ms)).await;
                    continue;
                }
            };

            let current_indexer_height = indexer_state.get_last_processed_block();
            if current_indexer_height >= current_chain_height {
                info!(
                    "Historical sync caught up to block {}. Switching to live or sleeping.",
                    current_indexer_height
                );
                sleep(Duration::from_millis(block_time_ms / 2)).await;
                continue;
            }

            let mut start_block = current_indexer_height;
            while start_block <= current_chain_height {
                let mut reconnect_attempt = 0;
                let batch_end = (start_block + batch_size).min(current_chain_height);

                info!(
                    "Processing blocks {} to {} (batch of {}) and lagging by {} blocks",
                    start_block,
                    batch_end,
                    batch_end - start_block + 1,
                    (current_chain_height - batch_end).max(0)
                );

                match self.get_historical_logs(start_block, batch_end).await {
                    Ok(logs) => {
                        if logs.is_empty() {
                            info!(
                                "No relevant logs found in blocks {} to {}",
                                start_block, batch_end
                            );
                        } else {
                            info!(
                                "Processing {} logs from blocks {} to {}",
                                logs.len(),
                                start_block,
                                batch_end
                            );

                            match self.process_logs(logs, batch_end).await {
                                Ok(_) => {
                                    info!(
                                        "Successfully processed and persisted logs for blocks {} to {}",
                                        start_block, batch_end
                                    );
                                }
                                Err(e) => {
                                    error!(
                                        "Error processing batch for blocks {} to {}: {:?}. Will retry this batch.",
                                        start_block, batch_end, e
                                    );
                                    // Implement retry logic with backoff, or halt if unrecoverable
                                    tokio::time::sleep(Duration::from_secs(5)).await;
                                    continue; // Retry the same batch
                                }
                            }
                        }

                        indexer_state.update_block(batch_end);
                        start_block = batch_end + 1;
                    }

                    Err(e) => {
                        if reconnect_attempt > MAX_RETRIES {
                            panic!(
                                "Max retries reached while getting logs from {:?} to {:?}, error: {:?}",
                                start_block, batch_end, e
                            );
                        }
                        reconnect_attempt += 1;

                        error!(
                            "Error while getting logs from {:?} to {:?}, error: {:?}",
                            start_block, batch_end, e
                        );

                        let delay = Duration::from_secs(5);

                        tokio::time::sleep(delay * 2u32.pow(reconnect_attempt as u32)).await;
                        continue; // Retry the same batch
                    }
                }
            }

            let sleep_duration = self.calculate_sleep_duration(
                current_indexer_height,
                current_chain_height,
                block_time_ms,
                None,
            );
            tokio::time::sleep(sleep_duration).await;

            continue;
        }
    }

    /// Calculate sleep duration based on actual block timing
    // #[instrument(skip(self), fields(chain_id = %self.handler.get_chain_config().chain_id))]
    fn calculate_sleep_duration(
        &self,
        current_block: u64,
        available_latest_block: u64,
        block_time_ms: u64,
        last_processed_block_info: Option<(u64, u64)>,
    ) -> Duration {
        if current_block >= available_latest_block {
            // Use the timestamp from the last processed block if available
            if let Some((_, timestamp)) = last_processed_block_info {
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                // Calculate when the next block should be produced
                let next_block_expected_time = timestamp + block_time_ms;

                if next_block_expected_time > current_time {
                    let time_until_next_block = next_block_expected_time - current_time;
                    // Cap sleep time to reasonable bounds
                    let sleep_ms = time_until_next_block.min(block_time_ms * 2).max(100);
                    return Duration::from_millis(sleep_ms);
                }
            }

            // Fallback: sleep for a portion of block time when caught up
            return Duration::from_millis(block_time_ms / 2);
        }

        // If we're behind, use shorter sleep intervals to catch up faster
        let blocks_behind = available_latest_block - current_block;

        if blocks_behind > 10 {
            // Far behind: very short sleep to catch up quickly
            Duration::from_millis(100)
        } else if blocks_behind > 5 {
            // Moderately behind: short sleep
            Duration::from_millis(block_time_ms / 4)
        } else {
            // Close to caught up: normal sleep
            Duration::from_millis(block_time_ms / 2)
        }
    }

    async fn process_logs(
        &self,
        logs: Vec<<Self::EventHandler as ChainEventHandler>::LogType>,
        max_seen_height: u64,
    ) -> eyre::Result<()> {
        let concurrency_limit =
            self.get_indexer_settings().max_concurrency_for_log_process as usize;
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
            debug!("No successfully prepared event data to process ");
            return Ok(());
        }

        debug!(
            "Successfully prepared events: {}",
            prepared_event_data_results.len()
        );

        match self
            .get_db_client()
            .process_bulk_l1_database_operations(prepared_event_data_results)
            .await
        {
            Ok(_) => {
                debug!("Succesfully updated the database for a batch of logs");
                self.get_db_client()
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
