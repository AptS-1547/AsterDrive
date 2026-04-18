//! 数据库迁移：`add_directory_lookup_indexes`。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Keep live directory listing, duplicate-name lookup, and ORDER BY name on one lookup path.
        manager
            .create_index(
                Index::create()
                    .name("idx_folders_user_deleted_parent_name")
                    .table(Folders::Table)
                    .col(Folders::UserId)
                    .col(Folders::DeletedAt)
                    .col(Folders::ParentId)
                    .col(Folders::Name)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_files_user_deleted_folder_name")
                    .table(Files::Table)
                    .col(Files::UserId)
                    .col(Files::DeletedAt)
                    .col(Files::FolderId)
                    .col(Files::Name)
                    .to_owned(),
            )
            .await?;

        // Match the recycle-bin pagination order so deleted rows can be read without full scans.
        manager
            .create_index(
                Index::create()
                    .name("idx_folders_user_deleted_at_id")
                    .table(Folders::Table)
                    .col(Folders::UserId)
                    .col((Folders::DeletedAt, IndexOrder::Desc))
                    .col((Folders::Id, IndexOrder::Asc))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_files_user_deleted_at_id")
                    .table(Files::Table)
                    .col(Files::UserId)
                    .col((Files::DeletedAt, IndexOrder::Desc))
                    .col((Files::Id, IndexOrder::Asc))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_files_user_deleted_at_id")
                    .table(Files::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_folders_user_deleted_at_id")
                    .table(Folders::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_files_user_deleted_folder_name")
                    .table(Files::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_folders_user_deleted_parent_name")
                    .table(Folders::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Folders {
    Table,
    Id,
    Name,
    ParentId,
    UserId,
    DeletedAt,
}

#[derive(DeriveIden)]
enum Files {
    Table,
    Id,
    Name,
    FolderId,
    UserId,
    DeletedAt,
}
