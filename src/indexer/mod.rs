mod evm;
mod svm;

use crate::config::IndexerConfig;
use async_trait::async_trait;
use eyre::Result;
use sea_orm::DatabaseConnection;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{error, info};

pub const MAX_RETRIES: i32 = 20;
pub const RETRY_DELAY: Duration = Duration::from_millis(5000);

#[async_trait]
pub trait ChainIndexer: Send + Sync {
    async fn new(
        http_rpc_url: String,
        ws_rpc_url: String,
        chain_id: u64,
        starting_block: u64,
        db: &DatabaseConnection,
        contract_addrs: Vec<String>,
    ) -> Result<Self>
    where
        Self: Sized;

    async fn run(&mut self) -> Result<()>;
    fn chain_id(&self) -> u64;
}

macro_rules! create_and_spawn_indexer {
    ($type:ty, $http_rpc_url:expr, $ws_rpc_url:expr, $chain_id:expr, $starting_block:expr, $db_conn:expr, $name:expr, $contracts:expr) => {{
        let mut indexer = <$type>::new(
            $http_rpc_url,
            $ws_rpc_url,
            $chain_id,
            $starting_block,
            &$db_conn,
            $contracts,
        )
        .await?;
        tokio::spawn(async move {
            info!("Starting {} indexer", $name);
            indexer.run().await
        })
    }};
}

pub async fn start_indexer(
    config: IndexerConfig,
    db_conn: DatabaseConnection,
) -> Result<(
    JoinHandle<Result<()>>,
    JoinHandle<Result<()>>,
    JoinHandle<Result<()>>,
)> {
    let evm_contracts = vec![
        config.l1_erc20_gateway_address.clone(),
        config.l1_message_queue_address.clone(),
        config.eth_twine_chain_address.clone(),
    ];

    let twine_contracts = vec![config.l2_twine_messenger_address.clone()];

    let svm_contracts = vec![
        config.tokens_gateway_program_address.clone(),
        config.twine_chain_program_address.clone(),
    ];

    let evm_handle = create_and_spawn_indexer!(
        evm::EthereumIndexer,
        config.evm.http_rpc_url,
        config.evm.ws_rpc_url,
        config.evm.chain_id,
        config.evm.start_block,
        db_conn,
        "EVM",
        evm_contracts
    );

    let twine_handle = create_and_spawn_indexer!(
        evm::TwineIndexer,
        config.twine.http_rpc_url,
        config.twine.ws_rpc_url,
        config.twine.chain_id,
        config.twine.start_block,
        db_conn,
        "Twine",
        twine_contracts
    );

    let svm_handle = create_and_spawn_indexer!(
        svm::SVMIndexer,
        config.solana.http_rpc_url,
        config.solana.ws_rpc_url,
        config.solana.chain_id,
        config.solana.start_block,
        db_conn,
        "SVM",
        svm_contracts
    );

    Ok((evm_handle, twine_handle, svm_handle))
}
