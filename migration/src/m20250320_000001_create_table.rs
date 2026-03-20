use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // users
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Users::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Users::Username)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Users::Email)
                            .string_len(255)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Users::PasswordHash)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Users::Role)
                            .string_len(16)
                            .not_null()
                            .default("user"),
                    )
                    .col(
                        ColumnDef::new(Users::Status)
                            .string_len(16)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(Users::StorageUsed)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Users::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Users::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // storage_policies
        manager
            .create_table(
                Table::create()
                    .table(StoragePolicies::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(StoragePolicies::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(StoragePolicies::Name)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StoragePolicies::DriverType)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StoragePolicies::Endpoint)
                            .string_len(512)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(StoragePolicies::Bucket)
                            .string_len(255)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(StoragePolicies::AccessKey)
                            .string_len(512)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(StoragePolicies::SecretKey)
                            .string_len(512)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(StoragePolicies::BasePath)
                            .string_len(512)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(StoragePolicies::MaxFileSize)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(StoragePolicies::AllowedTypes)
                            .text()
                            .not_null()
                            .default("[]"),
                    )
                    .col(
                        ColumnDef::new(StoragePolicies::Options)
                            .text()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(StoragePolicies::IsDefault)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(StoragePolicies::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StoragePolicies::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // user_storage_policies
        manager
            .create_table(
                Table::create()
                    .table(UserStoragePolicies::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserStoragePolicies::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserStoragePolicies::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserStoragePolicies::PolicyId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UserStoragePolicies::IsDefault)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(UserStoragePolicies::QuotaBytes)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(UserStoragePolicies::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserStoragePolicies::Table, UserStoragePolicies::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserStoragePolicies::Table, UserStoragePolicies::PolicyId)
                            .to(StoragePolicies::Table, StoragePolicies::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // folders
        manager
            .create_table(
                Table::create()
                    .table(Folders::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Folders::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Folders::Name).string_len(255).not_null())
                    .col(ColumnDef::new(Folders::ParentId).big_integer().null())
                    .col(ColumnDef::new(Folders::UserId).big_integer().not_null())
                    .col(ColumnDef::new(Folders::PolicyId).big_integer().null())
                    .col(
                        ColumnDef::new(Folders::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Folders::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Folders::Table, Folders::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Folders::Table, Folders::PolicyId)
                            .to(StoragePolicies::Table, StoragePolicies::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        // file_blobs
        manager
            .create_table(
                Table::create()
                    .table(FileBlobs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FileBlobs::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(FileBlobs::Hash).string_len(64).not_null())
                    .col(ColumnDef::new(FileBlobs::Size).big_integer().not_null())
                    .col(ColumnDef::new(FileBlobs::PolicyId).big_integer().not_null())
                    .col(
                        ColumnDef::new(FileBlobs::StoragePath)
                            .string_len(1024)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FileBlobs::RefCount)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(FileBlobs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FileBlobs::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(FileBlobs::Table, FileBlobs::PolicyId)
                            .to(StoragePolicies::Table, StoragePolicies::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_file_blobs_hash_policy")
                    .table(FileBlobs::Table)
                    .col(FileBlobs::Hash)
                    .col(FileBlobs::PolicyId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // files
        manager
            .create_table(
                Table::create()
                    .table(Files::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Files::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Files::Name).string_len(255).not_null())
                    .col(ColumnDef::new(Files::FolderId).big_integer().null())
                    .col(ColumnDef::new(Files::BlobId).big_integer().not_null())
                    .col(ColumnDef::new(Files::UserId).big_integer().not_null())
                    .col(ColumnDef::new(Files::MimeType).string_len(128).not_null())
                    .col(
                        ColumnDef::new(Files::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Files::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Files::Table, Files::FolderId)
                            .to(Folders::Table, Folders::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Files::Table, Files::BlobId)
                            .to(FileBlobs::Table, FileBlobs::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Files::Table, Files::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // system_config
        manager
            .create_table(
                Table::create()
                    .table(SystemConfig::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SystemConfig::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SystemConfig::Key)
                            .string_len(128)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(SystemConfig::Value).text().not_null())
                    .col(
                        ColumnDef::new(SystemConfig::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SystemConfig::UpdatedBy).big_integer().null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SystemConfig::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Files::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(FileBlobs::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Folders::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UserStoragePolicies::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(StoragePolicies::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    Username,
    Email,
    PasswordHash,
    Role,
    Status,
    StorageUsed,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum StoragePolicies {
    Table,
    Id,
    Name,
    DriverType,
    Endpoint,
    Bucket,
    AccessKey,
    SecretKey,
    BasePath,
    MaxFileSize,
    AllowedTypes,
    Options,
    IsDefault,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum UserStoragePolicies {
    Table,
    Id,
    UserId,
    PolicyId,
    IsDefault,
    QuotaBytes,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Folders {
    Table,
    Id,
    Name,
    ParentId,
    UserId,
    PolicyId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum FileBlobs {
    Table,
    Id,
    Hash,
    Size,
    PolicyId,
    StoragePath,
    RefCount,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Files {
    Table,
    Id,
    Name,
    FolderId,
    BlobId,
    UserId,
    MimeType,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum SystemConfig {
    Table,
    Id,
    Key,
    Value,
    UpdatedAt,
    UpdatedBy,
}
