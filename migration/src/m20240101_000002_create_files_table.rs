use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Files::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Files::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Files::UserId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Files::Filename)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Files::OriginalFilename)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Files::MimeType)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Files::Size)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Files::StoragePath)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Files::StorageBackend)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Files::Checksum)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Files::IsPublic)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(Files::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Files::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_files_user_id")
                            .from(Files::Table, Files::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Files::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Files {
    Table,
    Id,
    UserId,
    Filename,
    OriginalFilename,
    MimeType,
    Size,
    StoragePath,
    StorageBackend,
    Checksum,
    IsPublic,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
