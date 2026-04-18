//! 数据库迁移：`create_background_tasks`。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum BackgroundTasks {
    Table,
    Id,
    Kind,
    Status,
    CreatorUserId,
    TeamId,
    ShareId,
    DisplayName,
    PayloadJson,
    ResultJson,
    ProgressCurrent,
    ProgressTotal,
    StatusText,
    AttemptCount,
    MaxAttempts,
    NextRunAt,
    ProcessingStartedAt,
    StartedAt,
    FinishedAt,
    LastError,
    ExpiresAt,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BackgroundTasks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BackgroundTasks::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::Kind)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::Status)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::CreatorUserId)
                            .big_integer()
                            .null(),
                    )
                    .col(ColumnDef::new(BackgroundTasks::TeamId).big_integer().null())
                    .col(
                        ColumnDef::new(BackgroundTasks::ShareId)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::DisplayName)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::PayloadJson)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(BackgroundTasks::ResultJson).text().null())
                    .col(
                        ColumnDef::new(BackgroundTasks::ProgressCurrent)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::ProgressTotal)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::StatusText)
                            .string_len(255)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(BackgroundTasks::MaxAttempts)
                            .integer()
                            .not_null()
                            .default(3),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, BackgroundTasks::NextRunAt)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(
                            manager,
                            BackgroundTasks::ProcessingStartedAt,
                        )
                        .null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, BackgroundTasks::StartedAt)
                            .null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, BackgroundTasks::FinishedAt)
                            .null(),
                    )
                    .col(ColumnDef::new(BackgroundTasks::LastError).text().null())
                    .col(
                        crate::time::utc_date_time_column(manager, BackgroundTasks::ExpiresAt)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, BackgroundTasks::CreatedAt)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, BackgroundTasks::UpdatedAt)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_background_tasks_due")
                    .table(BackgroundTasks::Table)
                    .col(BackgroundTasks::Status)
                    .col(BackgroundTasks::NextRunAt)
                    .col(BackgroundTasks::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_background_tasks_processing")
                    .table(BackgroundTasks::Table)
                    .col(BackgroundTasks::Status)
                    .col(BackgroundTasks::ProcessingStartedAt)
                    .col(BackgroundTasks::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_background_tasks_personal")
                    .table(BackgroundTasks::Table)
                    .col(BackgroundTasks::CreatorUserId)
                    .col(BackgroundTasks::TeamId)
                    .col(BackgroundTasks::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_background_tasks_team")
                    .table(BackgroundTasks::Table)
                    .col(BackgroundTasks::TeamId)
                    .col(BackgroundTasks::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_background_tasks_expires_at")
                    .table(BackgroundTasks::Table)
                    .col(BackgroundTasks::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(BackgroundTasks::Table).to_owned())
            .await
    }
}
