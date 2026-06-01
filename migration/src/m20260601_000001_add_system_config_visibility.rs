//! Add consumer-side visibility to runtime configuration entries.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SystemConfig::Table)
                    .add_column(
                        ColumnDef::new(SystemConfig::Visibility)
                            .string_len(16)
                            .not_null()
                            .default("private"),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SystemConfig::Table)
                    .drop_column(SystemConfig::Visibility)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum SystemConfig {
    Table,
    Visibility,
}
