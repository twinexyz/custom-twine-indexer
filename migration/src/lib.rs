pub use sea_orm_migration::prelude::*;

mod m20250203_000001_create_tables;
mod svm_migrations;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250203_000001_create_tables::Migration),
            Box::new(svm_migrations::m20250203_000001_native_token_deposits::Migration),
            Box::new(svm_migrations::m20250203_000002_spl_token_deposits::Migration),
            Box::new(svm_migrations::m20250203_000003_native_token_withdrawls::Migration),
            Box::new(svm_migrations::m20250203_000004_spl_token_withdrawls::Migration),
            Box::new(svm_migrations::m20250203_000005_commit_batch::Migration),
            Box::new(svm_migrations::m20250203_000006_finalize_batch::Migration),
        ]
    }
}
