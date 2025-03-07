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
        ]
    }
}
