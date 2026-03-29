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
                            .text()
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
                        ColumnDef::new(UploadSessionParts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessionParts::UpdatedAt)
                            .timestamp_with_time_zone()
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
                    .name("idx_upload_session_parts_upload_id")
                    .table(UploadSessionParts::Table)
                    .col(UploadSessionParts::UploadId)
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
            .drop_index(
                Index::drop()
                    .name("idx_upload_session_parts_upload_id")
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
