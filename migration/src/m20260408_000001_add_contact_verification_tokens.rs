//! 数据库迁移：`add_contact_verification_tokens`。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    EmailVerifiedAt,
    PendingEmail,
}

#[derive(DeriveIden)]
enum ContactVerificationTokens {
    Table,
    Id,
    UserId,
    Channel,
    Purpose,
    Target,
    TokenHash,
    ExpiresAt,
    ConsumedAt,
    CreatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column(
                        crate::time::utc_date_time_column(manager, Users::EmailVerifiedAt).null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column(ColumnDef::new(Users::PendingEmail).string_len(255).null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_users_pending_email")
                    .table(Users::Table)
                    .col(Users::PendingEmail)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ContactVerificationTokens::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ContactVerificationTokens::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ContactVerificationTokens::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContactVerificationTokens::Channel)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContactVerificationTokens::Purpose)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContactVerificationTokens::Target)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContactVerificationTokens::TokenHash)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        crate::time::utc_date_time_column(
                            manager,
                            ContactVerificationTokens::ExpiresAt,
                        )
                        .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(
                            manager,
                            ContactVerificationTokens::ConsumedAt,
                        )
                        .null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(
                            manager,
                            ContactVerificationTokens::CreatedAt,
                        )
                        .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                ContactVerificationTokens::Table,
                                ContactVerificationTokens::UserId,
                            )
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_contact_verification_tokens_user_purpose")
                    .table(ContactVerificationTokens::Table)
                    .col(ContactVerificationTokens::UserId)
                    .col(ContactVerificationTokens::Channel)
                    .col(ContactVerificationTokens::Purpose)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_contact_verification_tokens_expires_at")
                    .table(ContactVerificationTokens::Table)
                    .col(ContactVerificationTokens::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE users SET email_verified_at = created_at WHERE email_verified_at IS NULL",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ContactVerificationTokens::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_users_pending_email")
                    .table(Users::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .drop_column(Users::PendingEmail)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .drop_column(Users::EmailVerifiedAt)
                    .to_owned(),
            )
            .await
    }
}
