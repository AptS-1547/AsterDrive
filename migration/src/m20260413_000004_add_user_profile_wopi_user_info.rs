//! 数据库迁移：`add_user_profile_wopi_user_info`。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum UserProfiles {
    Table,
    WopiUserInfo,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(UserProfiles::Table)
                    .add_column(
                        ColumnDef::new(UserProfiles::WopiUserInfo)
                            .string_len(1024)
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(UserProfiles::Table)
                    .drop_column(UserProfiles::WopiUserInfo)
                    .to_owned(),
            )
            .await
    }
}
