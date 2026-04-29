//! 数据库迁移：创建 follower 受 primary 托管的 ingress profile。

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum ManagedIngressProfiles {
    Table,
    Id,
    ProfileKey,
    Name,
    DriverType,
    Endpoint,
    Bucket,
    AccessKey,
    SecretKey,
    BasePath,
    MaxFileSize,
    IsDefault,
    DesiredRevision,
    AppliedRevision,
    LastError,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_connection().get_database_backend();
        let mut last_error = ColumnDef::new(ManagedIngressProfiles::LastError);
        last_error.text().not_null();
        if backend != DatabaseBackend::MySql {
            last_error.default("");
        }

        manager
            .create_table(
                Table::create()
                    .table(ManagedIngressProfiles::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ManagedIngressProfiles::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ManagedIngressProfiles::ProfileKey)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ManagedIngressProfiles::Name)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ManagedIngressProfiles::DriverType)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ManagedIngressProfiles::Endpoint)
                            .string_len(512)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(ManagedIngressProfiles::Bucket)
                            .string_len(255)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(ManagedIngressProfiles::AccessKey)
                            .string_len(512)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(ManagedIngressProfiles::SecretKey)
                            .string_len(512)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(ManagedIngressProfiles::BasePath)
                            .string_len(1024)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(ManagedIngressProfiles::MaxFileSize)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ManagedIngressProfiles::IsDefault)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(ManagedIngressProfiles::DesiredRevision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(ManagedIngressProfiles::AppliedRevision)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(last_error)
                    .col(
                        crate::time::utc_date_time_column(
                            manager,
                            ManagedIngressProfiles::CreatedAt,
                        )
                        .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(
                            manager,
                            ManagedIngressProfiles::UpdatedAt,
                        )
                        .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_managed_ingress_profiles_profile_key")
                    .table(ManagedIngressProfiles::Table)
                    .col(ManagedIngressProfiles::ProfileKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_managed_ingress_profiles_is_default")
                    .table(ManagedIngressProfiles::Table)
                    .col(ManagedIngressProfiles::IsDefault)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_managed_ingress_profiles_is_default")
                    .table(ManagedIngressProfiles::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_managed_ingress_profiles_profile_key")
                    .table(ManagedIngressProfiles::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(ManagedIngressProfiles::Table)
                    .to_owned(),
            )
            .await
    }
}
