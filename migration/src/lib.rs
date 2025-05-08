pub use sea_orm_migration::prelude::*;

mod m20250203_000001_create_l1_tables;
mod m20250306_000002_create_last_synced_table;
mod m20250310_000003_create_twine_tables;
mod m20250310_000004_create_twine_rollup_tables;
mod m20250507_085332_create_da_table;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250203_000001_create_l1_tables::Migration),
            Box::new(m20250306_000002_create_last_synced_table::Migration),
            Box::new(m20250310_000003_create_twine_tables::Migration),
            Box::new(m20250310_000004_create_twine_rollup_tables::Migration),
            Box::new(m20250507_085332_create_da_table::Migration),
        ]
    }
}