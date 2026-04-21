//! 数据库迁移：拆分远端节点主控档案、入站绑定与 enrollment 会话。

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum ManagedFollowers {
    Table,
    Id,
    Name,
    BaseUrl,
    AccessKey,
    SecretKey,
    Namespace,
    IsEnabled,
    LastCapabilities,
    LastError,
    LastCheckedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum FollowerEnrollmentSessions {
    Table,
    Id,
    ManagedFollowerId,
    TokenHash,
    AckTokenHash,
    ExpiresAt,
    RedeemedAt,
    AckedAt,
    InvalidatedAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum MasterBindings {
    Table,
    Id,
    Name,
    MasterUrl,
    AccessKey,
    SecretKey,
    Namespace,
    IngressPolicyId,
    IsEnabled,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum StoragePolicies {
    Table,
    Id,
    RemoteNodeId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_connection().get_database_backend();
        let mut last_capabilities = ColumnDef::new(ManagedFollowers::LastCapabilities);
        last_capabilities.text().not_null();
        if backend != DatabaseBackend::MySql {
            last_capabilities.default("{}");
        }

        let mut last_error = ColumnDef::new(ManagedFollowers::LastError);
        last_error.text().not_null();
        if backend != DatabaseBackend::MySql {
            last_error.default("");
        }

        manager
            .create_table(
                Table::create()
                    .table(ManagedFollowers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ManagedFollowers::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ManagedFollowers::Name)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ManagedFollowers::BaseUrl)
                            .string_len(512)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(ManagedFollowers::AccessKey)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ManagedFollowers::SecretKey)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ManagedFollowers::Namespace)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ManagedFollowers::IsEnabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(last_capabilities)
                    .col(last_error)
                    .col(
                        crate::time::utc_date_time_column(manager, ManagedFollowers::LastCheckedAt)
                            .null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, ManagedFollowers::CreatedAt)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, ManagedFollowers::UpdatedAt)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(FollowerEnrollmentSessions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FollowerEnrollmentSessions::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FollowerEnrollmentSessions::ManagedFollowerId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FollowerEnrollmentSessions::TokenHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FollowerEnrollmentSessions::AckTokenHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(
                            manager,
                            FollowerEnrollmentSessions::ExpiresAt,
                        )
                        .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(
                            manager,
                            FollowerEnrollmentSessions::RedeemedAt,
                        )
                        .null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(
                            manager,
                            FollowerEnrollmentSessions::AckedAt,
                        )
                        .null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(
                            manager,
                            FollowerEnrollmentSessions::InvalidatedAt,
                        )
                        .null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(
                            manager,
                            FollowerEnrollmentSessions::CreatedAt,
                        )
                        .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                FollowerEnrollmentSessions::Table,
                                FollowerEnrollmentSessions::ManagedFollowerId,
                            )
                            .to(ManagedFollowers::Table, ManagedFollowers::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MasterBindings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MasterBindings::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MasterBindings::Name)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MasterBindings::MasterUrl)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MasterBindings::AccessKey)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MasterBindings::SecretKey)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MasterBindings::Namespace)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MasterBindings::IngressPolicyId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MasterBindings::IsEnabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, MasterBindings::CreatedAt)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, MasterBindings::UpdatedAt)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MasterBindings::Table, MasterBindings::IngressPolicyId)
                            .to(StoragePolicies::Table, StoragePolicies::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_managed_followers_access_key")
                    .table(ManagedFollowers::Table)
                    .col(ManagedFollowers::AccessKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_managed_followers_namespace")
                    .table(ManagedFollowers::Table)
                    .col(ManagedFollowers::Namespace)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_follower_enrollment_sessions_managed_follower_id")
                    .table(FollowerEnrollmentSessions::Table)
                    .col(FollowerEnrollmentSessions::ManagedFollowerId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_follower_enrollment_sessions_token_hash")
                    .table(FollowerEnrollmentSessions::Table)
                    .col(FollowerEnrollmentSessions::TokenHash)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_follower_enrollment_sessions_ack_token_hash")
                    .table(FollowerEnrollmentSessions::Table)
                    .col(FollowerEnrollmentSessions::AckTokenHash)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_master_bindings_access_key")
                    .table(MasterBindings::Table)
                    .col(MasterBindings::AccessKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_master_bindings_ingress_policy_id")
                    .table(MasterBindings::Table)
                    .col(MasterBindings::IngressPolicyId)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(StoragePolicies::Table)
                    .add_column(
                        ColumnDef::new(StoragePolicies::RemoteNodeId)
                            .big_integer()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_storage_policies_remote_node_id")
                        .from(StoragePolicies::Table, StoragePolicies::RemoteNodeId)
                        .to(ManagedFollowers::Table, ManagedFollowers::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_index(
                Index::create()
                    .name("idx_storage_policies_remote_node_id")
                    .table(StoragePolicies::Table)
                    .col(StoragePolicies::RemoteNodeId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .name("fk_storage_policies_remote_node_id")
                        .table(StoragePolicies::Table)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .drop_index(
                Index::drop()
                    .name("idx_storage_policies_remote_node_id")
                    .table(StoragePolicies::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(StoragePolicies::Table)
                    .drop_column(StoragePolicies::RemoteNodeId)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_master_bindings_ingress_policy_id")
                    .table(MasterBindings::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_master_bindings_access_key")
                    .table(MasterBindings::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_follower_enrollment_sessions_ack_token_hash")
                    .table(FollowerEnrollmentSessions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_follower_enrollment_sessions_token_hash")
                    .table(FollowerEnrollmentSessions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_follower_enrollment_sessions_managed_follower_id")
                    .table(FollowerEnrollmentSessions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_managed_followers_namespace")
                    .table(ManagedFollowers::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_managed_followers_access_key")
                    .table(ManagedFollowers::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(MasterBindings::Table).to_owned())
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(FollowerEnrollmentSessions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(ManagedFollowers::Table).to_owned())
            .await
    }
}
