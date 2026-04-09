use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                DELETE FROM system_config
                WHERE key IN (
                    'mail_template_register_activation_text',
                    'mail_template_contact_change_confirmation_text',
                    'mail_template_password_reset_text',
                    'mail_template_password_reset_notice_text',
                    'mail_template_contact_change_notice_text'
                )
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
