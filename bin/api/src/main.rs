use common::config::{self, LoadFromEnv as _};
use database::connect::connect;
use eyre::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::ApiConfig::from_env()?;
    let db_conn = connect(&cfg.database.url).await?;
    info!("Connected to Database");
    api_lib::start_api(db_conn, cfg.port).await
}
