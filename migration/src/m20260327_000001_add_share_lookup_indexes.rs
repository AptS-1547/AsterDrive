use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx_shares_user_file")
                    .table(Shares::Table)
                    .col(Shares::UserId)
                    .col(Shares::FileId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_shares_user_folder")
                    .table(Shares::Table)
                    .col(Shares::UserId)
                    .col(Shares::FolderId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_shares_user_file")
                    .table(Shares::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_shares_user_folder")
                    .table(Shares::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Shares {
    Table,
    UserId,
    FileId,
    FolderId,
}
