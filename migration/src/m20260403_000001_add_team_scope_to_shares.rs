//! 数据库迁移：`add_team_scope_to_shares`。

use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Shares::Table)
                    .add_column(ColumnDef::new(Shares::TeamId).big_integer().null())
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_shares_team_id")
                        .from(Shares::Table, Shares::TeamId)
                        .to(Teams::Table, Teams::Id)
                        .on_delete(ForeignKeyAction::Cascade)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_index(
                Index::create()
                    .name("idx_shares_team_id")
                    .table(Shares::Table)
                    .col(Shares::TeamId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_shares_team_file")
                    .table(Shares::Table)
                    .col(Shares::TeamId)
                    .col(Shares::FileId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_shares_team_folder")
                    .table(Shares::Table)
                    .col(Shares::TeamId)
                    .col(Shares::FolderId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_shares_team_folder")
                    .table(Shares::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_shares_team_file")
                    .table(Shares::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_shares_team_id")
                    .table(Shares::Table)
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() != DatabaseBackend::Sqlite {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .name("fk_shares_team_id")
                        .table(Shares::Table)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(Shares::Table)
                    .drop_column(Shares::TeamId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Shares {
    Table,
    TeamId,
    FileId,
    FolderId,
}

#[derive(DeriveIden)]
enum Teams {
    Table,
    Id,
}
