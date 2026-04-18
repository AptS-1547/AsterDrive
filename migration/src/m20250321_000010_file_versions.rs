//! 数据库迁移：`file_versions`。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FileVersions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FileVersions::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FileVersions::FileId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FileVersions::BlobId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(FileVersions::Version).integer().not_null())
                    .col(ColumnDef::new(FileVersions::Size).big_integer().not_null())
                    .col(
                        crate::time::utc_date_time_column(manager, FileVersions::CreatedAt)
                            .not_null(),
                    )
                    // blob 生命周期由应用层 ref_count 管理，DB 层不级联删除；
                    // 若还有 file_version 引用该 blob，就阻止 blob 被删，作为兜底。
                    .foreign_key(
                        ForeignKey::create()
                            .from(FileVersions::Table, FileVersions::BlobId)
                            .to(FileBlobs::Table, FileBlobs::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_file_versions_file_id")
                    .table(FileVersions::Table)
                    .col(FileVersions::FileId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FileVersions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum FileVersions {
    Table,
    Id,
    FileId,
    BlobId,
    Version,
    Size,
    CreatedAt,
}

#[derive(DeriveIden)]
enum FileBlobs {
    Table,
    Id,
}
