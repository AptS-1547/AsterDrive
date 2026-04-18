//! 数据库迁移：`add_background_task_steps`。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum BackgroundTasks {
    Table,
    StepsJson,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BackgroundTasks::Table)
                    .add_column(ColumnDef::new(BackgroundTasks::StepsJson).text().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BackgroundTasks::Table)
                    .drop_column(BackgroundTasks::StepsJson)
                    .to_owned(),
            )
            .await
    }
}
