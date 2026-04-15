use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 删除旧表
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("webdav_locks"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        // 创建新表
        manager
            .create_table(
                Table::create()
                    .table(ResourceLocks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ResourceLocks::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ResourceLocks::Token)
                            .string()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(ResourceLocks::EntityType)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ResourceLocks::EntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ResourceLocks::Path).string().not_null())
                    .col(ColumnDef::new(ResourceLocks::OwnerId).big_integer().null())
                    .col(ColumnDef::new(ResourceLocks::OwnerInfo).text().null())
                    .col(
                        crate::time::utc_date_time_column(manager, ResourceLocks::TimeoutAt).null(),
                    )
                    .col(
                        ColumnDef::new(ResourceLocks::Shared)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(ResourceLocks::Deep)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, ResourceLocks::CreatedAt)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // 一个资源只能有一个锁
        manager
            .create_index(
                Index::create()
                    .name("idx_resource_locks_entity")
                    .table(ResourceLocks::Table)
                    .col(ResourceLocks::EntityType)
                    .col(ResourceLocks::EntityId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_resource_locks_path")
                    .table(ResourceLocks::Table)
                    .col(ResourceLocks::Path)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ResourceLocks::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ResourceLocks {
    Table,
    Id,
    Token,
    EntityType,
    EntityId,
    Path,
    OwnerId,
    OwnerInfo,
    TimeoutAt,
    Shared,
    Deep,
    CreatedAt,
}
