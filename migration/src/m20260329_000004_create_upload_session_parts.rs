//! 数据库迁移：`create_upload_session_parts`。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UploadSessionParts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UploadSessionParts::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UploadSessionParts::UploadId)
                            .string_len(36)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessionParts::PartNumber)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessionParts::Etag)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessionParts::Size)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, UploadSessionParts::CreatedAt)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, UploadSessionParts::UpdatedAt)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UploadSessionParts::Table, UploadSessionParts::UploadId)
                            .to(UploadSessions::Table, UploadSessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_upload_session_parts_upload_id_part_number")
                    .table(UploadSessionParts::Table)
                    .col(UploadSessionParts::UploadId)
                    .col(UploadSessionParts::PartNumber)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("uq_upload_session_parts_upload_id_part_number")
                    .table(UploadSessionParts::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(UploadSessionParts::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum UploadSessionParts {
    Table,
    Id,
    UploadId,
    PartNumber,
    Etag,
    Size,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum UploadSessions {
    Table,
    Id,
}
