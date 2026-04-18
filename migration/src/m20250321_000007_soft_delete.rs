//! 数据库迁移：`soft_delete`。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // files 表加 deleted_at
        manager
            .alter_table(
                Table::alter()
                    .table(Files::Table)
                    .add_column(crate::time::utc_date_time_column(manager, Files::DeletedAt).null())
                    .to_owned(),
            )
            .await?;

        // folders 表加 deleted_at
        manager
            .alter_table(
                Table::alter()
                    .table(Folders::Table)
                    .add_column(
                        crate::time::utc_date_time_column(manager, Folders::DeletedAt).null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Files::Table)
                    .drop_column(Files::DeletedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Folders::Table)
                    .drop_column(Folders::DeletedAt)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Files {
    Table,
    DeletedAt,
}

#[derive(DeriveIden)]
enum Folders {
    Table,
    DeletedAt,
}
