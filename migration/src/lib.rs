pub use sea_orm_migration::prelude::*;

mod m20250306_000002_create_last_synced_table;
mod m20250507_085332_create_da_table;
mod m20250530_074724_create_bridge_transaction_table;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250306_000002_create_last_synced_table::Migration),
            Box::new(m20250507_085332_create_da_table::Migration),
            Box::new(m20250530_074724_create_bridge_transaction_table::Migration),
        ]
    }
}