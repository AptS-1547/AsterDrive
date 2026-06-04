//! Allow multiple shared WebDAV locks on the same resource.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_resource_locks_entity")
                    .table(ResourceLocks::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_resource_locks_entity")
                    .table(ResourceLocks::Table)
                    .col(ResourceLocks::EntityType)
                    .col(ResourceLocks::EntityId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_resource_locks_entity")
                    .table(ResourceLocks::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

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
            .await
    }
}

#[derive(DeriveIden)]
enum ResourceLocks {
    Table,
    EntityType,
    EntityId,
}
