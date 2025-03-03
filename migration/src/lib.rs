pub use sea_orm_migration::prelude::*;

mod m20250203_000001_create_tables;
mod m20250203_000002_create_twine_transaction_batch;
mod m20250203_000003_create_twine_transaction_batch_detail;
mod m20250203_000004_create_twine_batch_l2_blocks;
mod m20250203_000005_create_twine_batch_l2_transactions;
mod m20250203_000006_create_twine_lifecycle_l1_transactions;
mod shared_enums;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250203_000001_create_tables::Migration),
            Box::new(m20250203_000002_create_twine_transaction_batch::Migration),
            Box::new(m20250203_000003_create_twine_transaction_batch_detail::Migration),
            Box::new(m20250203_000004_create_twine_batch_l2_blocks::Migration),
            Box::new(m20250203_000005_create_twine_batch_l2_transactions::Migration),
            Box::new(m20250203_000006_create_twine_lifecycle_l1_transactions::Migration),
        ]
    }
}
