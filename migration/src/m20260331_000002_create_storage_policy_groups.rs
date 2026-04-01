use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum StoragePolicyGroups {
    Table,
    Id,
    Name,
    Description,
    IsEnabled,
    IsDefault,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum StoragePolicyGroupItems {
    Table,
    Id,
    GroupId,
    PolicyId,
    Priority,
    MinFileSize,
    MaxFileSize,
    CreatedAt,
}

#[derive(DeriveIden)]
enum StoragePolicies {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    PolicyGroupId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(StoragePolicyGroups::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(StoragePolicyGroups::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(StoragePolicyGroups::Name)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StoragePolicyGroups::Description)
                            .string_len(512)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(StoragePolicyGroups::IsEnabled)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(StoragePolicyGroups::IsDefault)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(StoragePolicyGroups::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StoragePolicyGroups::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(StoragePolicyGroupItems::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(StoragePolicyGroupItems::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(StoragePolicyGroupItems::GroupId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StoragePolicyGroupItems::PolicyId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(StoragePolicyGroupItems::Priority)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(StoragePolicyGroupItems::MinFileSize)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(StoragePolicyGroupItems::MaxFileSize)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(StoragePolicyGroupItems::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                StoragePolicyGroupItems::Table,
                                StoragePolicyGroupItems::GroupId,
                            )
                            .to(StoragePolicyGroups::Table, StoragePolicyGroups::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                StoragePolicyGroupItems::Table,
                                StoragePolicyGroupItems::PolicyId,
                            )
                            .to(StoragePolicies::Table, StoragePolicies::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_spgi_group_priority")
                    .table(StoragePolicyGroupItems::Table)
                    .col(StoragePolicyGroupItems::GroupId)
                    .col(StoragePolicyGroupItems::Priority)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_spgi_group_policy")
                    .table(StoragePolicyGroupItems::Table)
                    .col(StoragePolicyGroupItems::GroupId)
                    .col(StoragePolicyGroupItems::PolicyId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column(ColumnDef::new(Users::PolicyGroupId).big_integer().null())
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_users_policy_group_id")
                        .from(Users::Table, Users::PolicyGroupId)
                        .to(StoragePolicyGroups::Table, StoragePolicyGroups::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_index(
                Index::create()
                    .name("idx_users_policy_group_id")
                    .table(Users::Table)
                    .col(Users::PolicyGroupId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_users_policy_group_id")
                    .table(Users::Table)
                    .to_owned(),
            )
            .await?;
        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .name("fk_users_policy_group_id")
                        .table(Users::Table)
                        .to_owned(),
                )
                .await?;
        }
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .drop_column(Users::PolicyGroupId)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_spgi_group_policy")
                    .table(StoragePolicyGroupItems::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_spgi_group_priority")
                    .table(StoragePolicyGroupItems::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(StoragePolicyGroupItems::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(StoragePolicyGroups::Table).to_owned())
            .await
    }
}
