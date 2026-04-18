//! 数据库迁移：`system_config_metadata`。

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
                        ColumnDef::new(SystemConfig::ValueType)
                            .string_len(32)
                            .not_null()
                            .default("string"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SystemConfig::Table)
                    .add_column(
                        ColumnDef::new(SystemConfig::RequiresRestart)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SystemConfig::Table)
                    .add_column(
                        ColumnDef::new(SystemConfig::IsSensitive)
                            .boolean()
                            .not_null()
                            .default(false),
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
                    .drop_column(SystemConfig::ValueType)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(SystemConfig::Table)
                    .drop_column(SystemConfig::RequiresRestart)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(SystemConfig::Table)
                    .drop_column(SystemConfig::IsSensitive)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum SystemConfig {
    Table,
    ValueType,
    RequiresRestart,
    IsSensitive,
}
