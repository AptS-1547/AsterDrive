//! 数据库迁移：`add_maintenance_indexes`。

use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx_upload_sessions_status_expires_at")
                    .table(UploadSessions::Table)
                    .col(UploadSessions::Status)
                    .col(UploadSessions::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_files_blob_id")
                    .table(Files::Table)
                    .col(Files::BlobId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_file_versions_blob_id")
                    .table(FileVersions::Table)
                    .col(FileVersions::BlobId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index({
                let mut index = Index::create();
                index
                    .name("idx_file_blobs_storage_path")
                    .table(FileBlobs::Table);

                if manager.get_connection().get_database_backend() == DbBackend::MySql {
                    index.col((FileBlobs::StoragePath, 255));
                } else {
                    index.col(FileBlobs::StoragePath);
                }

                index.to_owned()
            })
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_file_blobs_storage_path")
                    .table(FileBlobs::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_file_versions_blob_id")
                    .table(FileVersions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_files_blob_id")
                    .table(Files::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_upload_sessions_status_expires_at")
                    .table(UploadSessions::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum UploadSessions {
    Table,
    Status,
    ExpiresAt,
}

#[derive(DeriveIden)]
enum Files {
    Table,
    BlobId,
}

#[derive(DeriveIden)]
enum FileVersions {
    Table,
    BlobId,
}

#[derive(DeriveIden)]
enum FileBlobs {
    Table,
    StoragePath,
}
