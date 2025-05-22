use eyre::Result;
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};

pub async fn connect(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut opt = ConnectOptions::new(database_url.to_owned());
    opt.sqlx_logging(false); // Disable SQLx log

    Database::connect(opt).await
}
