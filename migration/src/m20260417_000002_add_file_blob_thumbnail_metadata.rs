//! 数据库迁移：`add_file_blob_thumbnail_metadata`。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum FileBlobs {
    Table,
    ThumbnailPath,
    ThumbnailVersion,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FileBlobs::Table)
                    .add_column(
                        ColumnDef::new(FileBlobs::ThumbnailPath)
                            .string_len(1024)
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(FileBlobs::Table)
                    .add_column(
                        ColumnDef::new(FileBlobs::ThumbnailVersion)
                            .string_len(32)
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(FileBlobs::Table)
                    .drop_column(FileBlobs::ThumbnailVersion)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(FileBlobs::Table)
                    .drop_column(FileBlobs::ThumbnailPath)
                    .to_owned(),
            )
            .await
    }
}
