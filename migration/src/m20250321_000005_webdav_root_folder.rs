//! 数据库迁移：`webdav_root_folder`。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(WebdavAccounts::Table)
                    .add_column(
                        ColumnDef::new(WebdavAccounts::RootFolderId)
                            .big_integer()
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
                    .table(WebdavAccounts::Table)
                    .drop_column(WebdavAccounts::RootFolderId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum WebdavAccounts {
    Table,
    RootFolderId,
}
