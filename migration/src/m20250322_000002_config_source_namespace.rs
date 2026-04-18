//! 数据库迁移：`config_source_namespace`。

use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_connection().get_database_backend();
        // source: "system" | "custom"
        manager
            .alter_table(
                Table::alter()
                    .table(SystemConfig::Table)
                    .add_column(
                        ColumnDef::new(SystemConfig::Source)
                            .string_len(16)
                            .not_null()
                            .default("system"),
                    )
                    .to_owned(),
            )
            .await?;

        // namespace: 自定义配置的命名空间，系统配置为 ""
        manager
            .alter_table(
                Table::alter()
                    .table(SystemConfig::Table)
                    .add_column(
                        ColumnDef::new(SystemConfig::Namespace)
                            .string_len(128)
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        // category: 分类（前端分组用）
        manager
            .alter_table(
                Table::alter()
                    .table(SystemConfig::Table)
                    .add_column(
                        ColumnDef::new(SystemConfig::Category)
                            .string_len(64)
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        // description: 描述
        let mut description = ColumnDef::new(SystemConfig::Description);
        description.text().not_null();
        if backend != DbBackend::MySql {
            description.default("");
        }

        manager
            .alter_table(
                Table::alter()
                    .table(SystemConfig::Table)
                    .add_column(description)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for col in [
            SystemConfig::Description,
            SystemConfig::Category,
            SystemConfig::Namespace,
            SystemConfig::Source,
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(SystemConfig::Table)
                        .drop_column(col)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[derive(DeriveIden)]
enum SystemConfig {
    Table,
    Source,
    Namespace,
    Category,
    Description,
}
