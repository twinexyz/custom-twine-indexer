use std::{
    f64::consts::E,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use alloy::{primitives::U64, rpc::types::Log};
use common::config::{ChainConfig, EvmConfig};
use database::{client::DbClient, entities::last_synced, DbOperations};
use eyre::{eyre, Error};
use futures_util::StreamExt;
use tokio::{
    spawn,
    sync::{watch, Semaphore},
    task::JoinSet,
    time::Instant,
};
use tracing::{error, info, instrument};

use crate::{handler::EvmEventHandler, provider::EvmProvider};

pub struct EvmIndexer<H: EvmEventHandler> {
    provider: EvmProvider,
    handler: H,
    db: Arc<DbClient>,
    config: ChainConfig,
}

impl<H: EvmEventHandler> EvmIndexer<H> {
    pub async fn new(handler: H, db: Arc<DbClient>) -> Result<Self, Error> {
        let config = handler.get_chain_config();
        let provider =
            EvmProvider::new(&config.http_rpc_url, &config.ws_rpc_url, config.chain_id).await?;

        Ok(Self {
            provider,
            handler,
            db,
            config,
            // state: Arc::new(AtomicU64::new(1)),
        })
    }

    pub async fn run(&mut self) -> Result<(), Error> {
        let initial_block = self.initialize_state().await?;

        match self.sync_historical(initial_block).await {
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

    async fn initialize_state(&self) -> Result<u64, Error> {
        let last_synced = self
            .db
            .get_last_synced_height(self.handler.chain_id() as i64, self.config.start_block)
            .await
            .unwrap_or(self.config.start_block as i64);
        Ok(last_synced as u64)
    }

    #[instrument(skip_all, fields(CHAIN = %self.handler.chain_id()))]
    async fn sync_historical(&self, from: u64) -> Result<(), Error> {
        let current_block = self.provider.get_block_number().await?;
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
            let end_block = (start_block + self.config.block_sync_batch_size).min(current_block);

            info!(
                "Missing blocks: {} and Fetching logs for blocks {} to {}",
                current_block - start_block,
                start_block,
                end_block
            );

            match self
                .provider
                .get_logs(
                    &self.handler.relevant_addresses(),
                    &self.handler.relevant_topics(),
                    start_block,
                    end_block,
                )
                .await
            {
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
                                error!("Error processing batch for blocks {} to {}: {:?}. Will retry this batch.", start_block, end_block, e);
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
                        "Error while getting logs from {:?} to {:?}",
                        start_block, end_block
                    );

                    let delay = Duration::from_secs(5);

                    tokio::time::sleep(delay * 2u32.pow(reconnect_attempt)).await;
                    continue; // Retry the same batch
                }
            }
        }
        Ok(())
    }

    async fn sync_live(&self) -> Result<(), Error> {
        //TODO: Make it configurable
        const MAX_BATCH_SIZE: usize = 1000;
        const MAX_BATCH_TIME: Duration = Duration::from_secs(15);

        let mut reconnect_attempts = 0;

        loop {
            let mut last_synced = self
                .db
                .get_last_synced_height(self.handler.chain_id() as i64, self.config.start_block)
                .await?;

            match self
                .provider
                .subscribe_logs(
                    &self.handler.relevant_addresses(),
                    &self.handler.relevant_topics(),
                    Some(last_synced as u64),
                )
                .await
            {
                Ok(mut stream) => {
                    info!("Subscribed to live logs");
                    reconnect_attempts = 0;

                    let mut buffer = Vec::with_capacity(MAX_BATCH_SIZE);
                    let mut last_flush = Instant::now();
                    let mut max_seen_block = last_synced;

                    loop {
                        let sleep_duration = MAX_BATCH_TIME.saturating_sub(last_flush.elapsed());
                        let sleep_future = tokio::time::sleep(sleep_duration);
                        tokio::pin!(sleep_future);

                        tokio::select! {
                            maybe_log = stream.next() => {
                                match maybe_log {
                                    Some(log) => {
                                        if let Some(block_number) = log.block_number {
                                           max_seen_block = max_seen_block.max(block_number as i64);
                                        }

                                        buffer.push(log);

                                        if buffer.len() >= MAX_BATCH_SIZE  {
                                            self.process_buffer(&mut buffer, max_seen_block).await?;
                                            last_flush = Instant::now();
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

        Ok(())
    }

    async fn process_buffer(
        &self,
        buffer: &mut Vec<Log>,
        max_seen_block: i64,
    ) -> Result<(), Error> {
        let logs = std::mem::replace(buffer, Vec::with_capacity(100));
        self.process_logs(logs, max_seen_block as u64).await?;
        Ok(())
    }

    async fn process_logs(&self, logs: Vec<Log>, max_seen_block: u64) -> Result<(), Error> {
        //Main logic of processing logs
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
            return Err(eyre!(
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
            .db
            .process_bulk_l1_database_operations(prepared_event_data_results)
            .await
        {
            Ok(_) => {
                info!("Succesfully updated the database");
                self.db
                    .upsert_last_synced(self.handler.chain_id() as i64, max_seen_block as i64)
                    .await?;
            }

            Err(e) => {
                return Err(eyre!("Error while bulking database operation {:?}", e));
            }
        }

        Ok(())
    }
}

impl<H: EvmEventHandler> Clone for EvmIndexer<H> {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            handler: self.handler.clone(),
            db: self.db.clone(),
            config: self.config.clone(),
        }
    }
}
