/// Manages the indexer's state including last processed block and persistence tracking
#[derive(Debug, Clone)]
pub struct IndexerState {
    last_processed_block: u64,
    chain_id: u64,
    checkpoint_interval: u64, // Number of blocks between database checkpoints
    blocks_since_last_checkpoint: u64,
}

impl IndexerState {
    pub fn new(initial_block: u64, chain_id: u64, block_time_ms: u64) -> Self {
        // Set checkpoint interval to approximately 15 minutes
        let checkpoint_interval = (15 * 60 * 1000) / block_time_ms;
        Self {
            last_processed_block: initial_block,
            chain_id,
            checkpoint_interval: checkpoint_interval.max(1), // At least 1 block
            blocks_since_last_checkpoint: 0,
        }
    }

    pub fn get_last_processed_block(&self) -> u64 {
        self.last_processed_block
    }

    /// Update block and mark if data was processed
    pub fn update_block(&mut self, new_block: u64) {
        self.last_processed_block = new_block;
        self.blocks_since_last_checkpoint += 1;
    }

    /// Check if we should persist checkpoint (either time-based or data-based)
    pub fn should_persist_checkpoint(&self) -> bool {
        self.blocks_since_last_checkpoint >= self.checkpoint_interval
    }

    /// Reset checkpoint counter and data flag after persistence
    pub fn reset_checkpoint_state(&mut self) {
        self.blocks_since_last_checkpoint = 0;
    }

    pub fn get_chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Get time-based checkpoint interval for reference
    pub fn get_checkpoint_interval(&self) -> u64 {
        self.checkpoint_interval
    }
}
