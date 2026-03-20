pub use sea_orm_migration::prelude::*;

mod m20250320_000001_create_table;
mod m20250321_000001_add_storage_quota;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250320_000001_create_table::Migration),
            Box::new(m20250321_000001_add_storage_quota::Migration),
        ]
    }
}
