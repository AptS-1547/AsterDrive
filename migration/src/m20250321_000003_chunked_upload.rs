//! 数据库迁移：`chunked_upload`。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. storage_policies 加 chunk_size 列
        manager
            .alter_table(
                Table::alter()
                    .table(StoragePolicies::Table)
                    .add_column(
                        ColumnDef::new(StoragePolicies::ChunkSize)
                            .big_integer()
                            .not_null()
                            .default(5_242_880i64), // 5MB
                    )
                    .to_owned(),
            )
            .await?;

        // 2. 创建 upload_sessions 表
        manager
            .create_table(
                Table::create()
                    .table(UploadSessions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UploadSessions::Id)
                            .string_len(36)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::Filename)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::TotalSize)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::ChunkSize)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::TotalChunks)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::ReceivedCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::FolderId)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::PolicyId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::Status)
                            .string_len(16)
                            .not_null()
                            .default("uploading"),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, UploadSessions::CreatedAt)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, UploadSessions::ExpiresAt)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, UploadSessions::UpdatedAt)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UploadSessions::Table, UploadSessions::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UploadSessions::Table).to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(StoragePolicies::Table)
                    .drop_column(StoragePolicies::ChunkSize)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum StoragePolicies {
    Table,
    ChunkSize,
}

#[derive(DeriveIden)]
enum UploadSessions {
    Table,
    Id,
    UserId,
    Filename,
    TotalSize,
    ChunkSize,
    TotalChunks,
    ReceivedCount,
    FolderId,
    PolicyId,
    Status,
    CreatedAt,
    ExpiresAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
