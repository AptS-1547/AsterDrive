//! 数据库迁移：`create_mail_outbox`。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum MailOutbox {
    Table,
    Id,
    TemplateCode,
    ToAddress,
    ToName,
    PayloadJson,
    Status,
    AttemptCount,
    NextAttemptAt,
    ProcessingStartedAt,
    SentAt,
    LastError,
    CreatedAt,
    UpdatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MailOutbox::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MailOutbox::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MailOutbox::TemplateCode)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MailOutbox::ToAddress)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MailOutbox::ToName).string_len(255).null())
                    .col(ColumnDef::new(MailOutbox::PayloadJson).text().not_null())
                    .col(ColumnDef::new(MailOutbox::Status).string_len(16).not_null())
                    .col(
                        ColumnDef::new(MailOutbox::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, MailOutbox::NextAttemptAt)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, MailOutbox::ProcessingStartedAt)
                            .null(),
                    )
                    .col(crate::time::utc_date_time_column(manager, MailOutbox::SentAt).null())
                    .col(ColumnDef::new(MailOutbox::LastError).text().null())
                    .col(
                        crate::time::utc_date_time_column(manager, MailOutbox::CreatedAt)
                            .not_null(),
                    )
                    .col(
                        crate::time::utc_date_time_column(manager, MailOutbox::UpdatedAt)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_mail_outbox_due")
                    .table(MailOutbox::Table)
                    .col(MailOutbox::Status)
                    .col(MailOutbox::NextAttemptAt)
                    .col(MailOutbox::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_mail_outbox_processing")
                    .table(MailOutbox::Table)
                    .col(MailOutbox::Status)
                    .col(MailOutbox::ProcessingStartedAt)
                    .col(MailOutbox::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_mail_outbox_sent_at")
                    .table(MailOutbox::Table)
                    .col(MailOutbox::SentAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MailOutbox::Table).to_owned())
            .await
    }
}
