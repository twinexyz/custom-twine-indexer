use eyre::Result;
use tracing::{error, info};
use twine_indexer::{config, db, indexer};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::IndexerConfig::from_env()?;
    let db_conn = db::connect(&cfg.database_url).await?;
    info!("Connected to Database");

    let (evm_handle, twine_handle, svm_handle) = indexer::start_indexer(cfg, db_conn).await?;

    // Join the tasks and handle any panics
    match tokio::try_join!(evm_handle, twine_handle, svm_handle) {
        Ok((evm_result, twine_result, svm_result)) => {
            evm_result?;
            twine_result?;
            svm_result?;
            Ok(())
        }
        Err(e) => {
            if e.is_panic() {
                error!("A task panicked: {:?}", e);
                Err(eyre::eyre!("A task panicked: {:?}", e))
            } else {
                error!("A task was cancelled: {:?}", e);
                Err(eyre::eyre!("A task was cancelled: {:?}", e))
            }
        }
    }
}
