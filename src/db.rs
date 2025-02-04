use crate::entities::l1_deposit;
use eyre::Result;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Schema};

pub async fn connect(database_url: &str) -> Result<DatabaseConnection, sea_orm::DbErr> {
    Database::connect(database_url).await
}
