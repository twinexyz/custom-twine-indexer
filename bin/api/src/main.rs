use common::{config, db};
use eyre::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cfg = config::ApiConfig::from_env()?;
    let db_conn = db::connect(&cfg.database_url).await?;
    info!("Connected to Database");
    api_lib::start_api(db_conn, cfg.api_port).await
}
