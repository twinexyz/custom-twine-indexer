use std::{sync::Arc, time::Duration};

use crate::handler::ChainEventHandler;
use async_trait::async_trait;
use common::config::IndexerSettings;
use database::client::DbClient;
use eyre::Error;
use futures_util::{Stream, StreamExt};
use tokio::{sync::Semaphore, task::JoinSet, time::Instant};
use tracing::{debug, error, info, instrument};

#[async_trait]
pub trait ChainIndexer: Send + Sync {
    type EventHandler: ChainEventHandler + Clone + Send + Sync + 'static;
    type LiveStream: Stream<Item = eyre::Result<<Self::EventHandler as ChainEventHandler>::LogType>>
        + Send
        + 'static;

    async fn get_initial_state(&self) -> eyre::Result<u64>;

    async fn get_historical_logs(
        &self,
        from: u64,
        to: u64,
    ) -> eyre::Result<Vec<<Self::EventHandler as ChainEventHandler>::LogType>>;

    async fn subscribe_live(&self, from_block: Option<u64>) -> eyre::Result<Self::LiveStream>;

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
    async fn sync_live(&self) -> Result<(), Error> {
        let settings = self.get_indexer_settings();

        let max_batch_size: usize = settings.max_log_batch_size as usize;
        let max_batch_time: Duration = Duration::from_secs(settings.max_log_batch_time);

        let mut reconnect_attempts = 0;

        loop {
            let last_synced = self
                .get_db_client()
                .get_last_synced_height(self.get_event_handler().chain_id() as i64, 0)
                .await?;

            if reconnect_attempts > 5 {
                info!("Switching to historical from live sync");
                self.sync_historical(last_synced as u64).await?;
                continue;
            }

            match self.subscribe_live(Some(last_synced as u64)).await {
                Ok(stream) => {
                    info!("Subscribed to live logs");
                    reconnect_attempts = 0;

                    let mut buffer = Vec::with_capacity(max_batch_size);
                    let mut last_flush = tokio::time::Instant::now();
                    let mut max_seen_block = last_synced;

                    let mut value = Box::pin(stream);
                    loop {
                        let sleep_duration = max_batch_time.saturating_sub(last_flush.elapsed());
                        let sleep_future = tokio::time::sleep(sleep_duration);
                        tokio::pin!(sleep_future);

                        tokio::select! {
                            maybe_log = value.next() => {
                                match maybe_log {
                                    Some(log) => {
                                        if let Ok(log) = log {
                                            let log_block_number = self.get_block_number_from_log(&log.clone());



                                            if let Some(block_number) = log_block_number {
                                           max_seen_block = max_seen_block.max(block_number as i64);
                                        }

                                        buffer.push(log);

                                            let mut valid_logs: Vec<_> = buffer.clone();

                                            if valid_logs.len() >= max_batch_size {
                                                self.process_buffer(&mut valid_logs, max_seen_block).await?;
                                                // self.process_buffer(&mut buffer, max_seen_block).await?;
                                                last_flush = Instant::now();
                                            }
                                        }

                                    }
                                    None => {
                                        info!("Stream closed. Reconnecting...");
                                        break;
                                    }
                                }
                            }

                            _ = &mut sleep_future => {
                                if !buffer.is_empty() {
                                    self.process_buffer(&mut buffer, max_seen_block).await?;
                                    last_flush = Instant::now();
                                    max_seen_block = 0;
                                }
                            }
                        }
                    }
                }

                Err(e) => {
                    let delay = Duration::from_secs(5);

                    reconnect_attempts += 1;
                    error!(
                        "Error subscribing to logs: {:?}. Reconnect attemp:{:?}, Retrying in {:?}",
                        e, reconnect_attempts, delay
                    );
                    tokio::time::sleep(delay * 2u32.pow(reconnect_attempts)).await;
                }
            }
        }
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
