use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const PROCESSING_HEARTBEAT_INDEX_NAME: &str = "idx_background_tasks_processing_heartbeat";
const PROCESSING_LEASE_INDEX_NAME: &str = "idx_background_tasks_processing_lease";

#[derive(DeriveIden)]
enum BackgroundTasks {
    Table,
    Status,
    CreatedAt,
    ProcessingToken,
    LastHeartbeatAt,
    LeaseExpiresAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(BackgroundTasks::Table)
                    .add_column(
                        ColumnDef::new(BackgroundTasks::ProcessingToken)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BackgroundTasks::Table)
                    .add_column(
                        crate::time::utc_date_time_column(
                            manager,
                            BackgroundTasks::LastHeartbeatAt,
                        )
                        .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BackgroundTasks::Table)
                    .add_column(
                        crate::time::utc_date_time_column(manager, BackgroundTasks::LeaseExpiresAt)
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(PROCESSING_HEARTBEAT_INDEX_NAME)
                    .table(BackgroundTasks::Table)
                    .col(BackgroundTasks::Status)
                    .col(BackgroundTasks::LastHeartbeatAt)
                    .col(BackgroundTasks::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(PROCESSING_LEASE_INDEX_NAME)
                    .table(BackgroundTasks::Table)
                    .col(BackgroundTasks::Status)
                    .col(BackgroundTasks::LeaseExpiresAt)
                    .col(BackgroundTasks::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(PROCESSING_LEASE_INDEX_NAME)
                    .table(BackgroundTasks::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name(PROCESSING_HEARTBEAT_INDEX_NAME)
                    .table(BackgroundTasks::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BackgroundTasks::Table)
                    .drop_column(BackgroundTasks::ProcessingToken)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BackgroundTasks::Table)
                    .drop_column(BackgroundTasks::LastHeartbeatAt)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(BackgroundTasks::Table)
                    .drop_column(BackgroundTasks::LeaseExpiresAt)
                    .to_owned(),
            )
            .await
    }
}
