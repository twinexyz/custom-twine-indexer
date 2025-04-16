use common::{config, db, indexer::ChainIndexer};
use eyre::{Result, WrapErr};
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;
use tracing::{error, info};

macro_rules! create_and_spawn_indexer {
    ($type:ty, $http_rpc_url:expr, $ws_rpc_url:expr, $chain_id:expr, $starting_block:expr, $db_conn:expr, $blockscout_db_conn:expr,  $name:expr, $contracts:expr) => {{
        let mut indexer = <$type>::new(
            $http_rpc_url,
            $ws_rpc_url,
            $chain_id,
            $starting_block,
            &$db_conn,
            $blockscout_db_conn,
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
    config: config::IndexerConfig,
    db_conn: DatabaseConnection,
    blockscout_db_conn: DatabaseConnection,
) -> Result<Vec<JoinHandle<Result<()>>>> {
    let mut handles = Vec::new();

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

    handles.push(create_and_spawn_indexer!(
        evm::EthereumIndexer,
        config.ethereum.http_rpc_url,
        config.ethereum.ws_rpc_url,
        config.ethereum.chain_id,
        config.ethereum.start_block,
        db_conn,
        Some(&blockscout_db_conn),
        "EVM",
        evm_contracts
    ));

    handles.push(create_and_spawn_indexer!(
        evm::TwineIndexer,
        config.twine.http_rpc_url,
        config.twine.ws_rpc_url,
        config.twine.chain_id,
        config.twine.start_block,
        db_conn,
        None,
        "Twine",
        twine_contracts
    ));

    handles.push(create_and_spawn_indexer!(
        svm::SVMIndexer,
        config.solana.http_rpc_url,
        config.solana.ws_rpc_url,
        config.solana.chain_id,
        config.solana.start_block,
        db_conn,
        Some(&blockscout_db_conn),
        "SVM",
        svm_contracts
    ));

    Ok(handles)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::IndexerConfig::from_env()?;
    let db_conn = db::connect(&cfg.database_url).await?;
    info!("Connected to Indexer's DB");
    let blockscout_db_conn = db::connect(&cfg.blockscout_database_url).await?;
    info!("Connected to Blockscout's DB");

    let handles = start_indexer(cfg, db_conn, blockscout_db_conn)
        .await
        .wrap_err("Failed to start indexers")?;

    for handle in handles {
        match handle.await {
            Ok(inner_result) => {
                if let Err(e) = inner_result {
                    error!("Indexer task failed: {:?}", e);
                }
            }
            Err(e) => {
                if e.is_panic() {
                    error!("A task panicked: {:?}", e);
                } else {
                    error!("A task was cancelled: {:?}", e);
                }
            }
        }
    }

    Ok(())
}
