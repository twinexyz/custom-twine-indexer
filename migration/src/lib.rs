pub use sea_orm_migration::prelude::*;

mod m20250203_000001_create_tables;
mod m20250306_000002_create_last_synced_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250203_000001_create_tables::Migration),
            Box::new(m20250306_000002_create_last_synced_table::Migration),
        ]
    }
}
