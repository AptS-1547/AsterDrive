use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum UserProfiles {
    Table,
    UserId,
    AvatarSource,
    AvatarPolicyId,
    AvatarKey,
    AvatarVersion,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum StoragePolicies {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserProfiles::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserProfiles::UserId)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserProfiles::AvatarSource)
                            .string_len(16)
                            .not_null()
                            .default("none"),
                    )
                    .col(
                        ColumnDef::new(UserProfiles::AvatarPolicyId)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(UserProfiles::AvatarKey)
                            .string_len(512)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(UserProfiles::AvatarVersion)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, UserProfiles::CreatedAt)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, UserProfiles::UpdatedAt)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserProfiles::Table, UserProfiles::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(UserProfiles::Table, UserProfiles::AvatarPolicyId)
                            .to(StoragePolicies::Table, StoragePolicies::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(UserProfiles::Table).to_owned())
            .await
    }
}
