//! 数据库迁移：`s3_multipart_upload`。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(UploadSessions::Table)
                    .add_column(ColumnDef::new(UploadSessions::S3MultipartId).text().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(UploadSessions::Table)
                    .drop_column(UploadSessions::S3MultipartId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum UploadSessions {
    Table,
    S3MultipartId,
}
