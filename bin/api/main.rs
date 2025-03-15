//- Only a comment
use eyre::Result;
use tracing::info;
use twine_indexer::{api, config, db};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::Config::from_env()?;
    let db_conn = db::connect(&cfg.database_url).await?;
    info!("Connected to Database");
    api::start_api(db_conn, cfg.api_port).await
}
