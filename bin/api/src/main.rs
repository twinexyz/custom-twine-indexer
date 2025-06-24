use common::config::{self, LoadFromEnv as _};
use database::connect::connect;
use eyre::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::ApiConfig::load()?;
    let primary_db_conn = connect(&cfg.database.url).await?;
    info!("Connected to Primary Database");
    api_lib::start_api(primary_db_conn, cfg.port).await
}
