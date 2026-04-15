use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Files {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum WopiSessions {
    Table,
    Id,
    TokenHash,
    ActorUserId,
    SessionVersion,
    TeamId,
    FileId,
    AppKey,
    ExpiresAt,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WopiSessions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(WopiSessions::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(WopiSessions::TokenHash)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(WopiSessions::ActorUserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WopiSessions::SessionVersion)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(WopiSessions::TeamId).big_integer().null())
                    .col(
                        ColumnDef::new(WopiSessions::FileId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(WopiSessions::AppKey)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, WopiSessions::ExpiresAt)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, WopiSessions::CreatedAt)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(WopiSessions::Table, WopiSessions::ActorUserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(WopiSessions::Table, WopiSessions::FileId)
                            .to(Files::Table, Files::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_wopi_sessions_expires_at")
                    .table(WopiSessions::Table)
                    .col(WopiSessions::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WopiSessions::Table).to_owned())
            .await
    }
}
