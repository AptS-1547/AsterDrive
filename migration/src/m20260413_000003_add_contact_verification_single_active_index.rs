//! 数据库迁移：`add_contact_verification_single_active_index`。

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        cleanup_duplicate_unconsumed_tokens(manager).await?;

        let db = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Sqlite => {
                db.execute_unprepared(
                    "CREATE UNIQUE INDEX idx_contact_verification_tokens_single_active \
                     ON contact_verification_tokens ( \
                        user_id, \
                        channel, \
                        purpose, \
                        (CASE WHEN consumed_at IS NULL THEN 1 ELSE NULL END) \
                     );",
                )
                .await?;
            }
            DatabaseBackend::Postgres => {
                db.execute_unprepared(
                    "CREATE UNIQUE INDEX idx_contact_verification_tokens_single_active \
                     ON contact_verification_tokens ( \
                        user_id, \
                        channel, \
                        purpose, \
                        (CASE WHEN consumed_at IS NULL THEN 1 ELSE NULL END) \
                     );",
                )
                .await?;
            }
            DatabaseBackend::MySql => {
                db.execute_unprepared(
                    "CREATE UNIQUE INDEX idx_contact_verification_tokens_single_active \
                     ON contact_verification_tokens ( \
                        user_id, \
                        channel, \
                        purpose, \
                        ((CASE WHEN consumed_at IS NULL THEN 1 ELSE NULL END)) \
                     );",
                )
                .await?;
            }
            _ => {
                return Err(DbErr::Migration(
                    "unsupported database backend for contact verification active index"
                        .to_string(),
                ));
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        match manager.get_database_backend() {
            DatabaseBackend::Sqlite | DatabaseBackend::Postgres => {
                db.execute_unprepared(
                    "DROP INDEX IF EXISTS idx_contact_verification_tokens_single_active;",
                )
                .await?;
            }
            DatabaseBackend::MySql => {
                db.execute_unprepared(
                    "DROP INDEX idx_contact_verification_tokens_single_active \
                     ON contact_verification_tokens;",
                )
                .await?;
            }
            _ => {
                return Err(DbErr::Migration(
                    "unsupported database backend for contact verification active index"
                        .to_string(),
                ));
            }
        }

        Ok(())
    }
}

async fn cleanup_duplicate_unconsumed_tokens(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();

    match manager.get_database_backend() {
        DatabaseBackend::Sqlite | DatabaseBackend::Postgres => {
            db.execute_unprepared(
                "DELETE FROM contact_verification_tokens AS current_row \
                 WHERE current_row.consumed_at IS NULL \
                   AND EXISTS ( \
                       SELECT 1 \
                       FROM contact_verification_tokens AS newer \
                       WHERE newer.user_id = current_row.user_id \
                         AND newer.channel = current_row.channel \
                         AND newer.purpose = current_row.purpose \
                         AND newer.consumed_at IS NULL \
                         AND ( \
                               newer.created_at > current_row.created_at \
                            OR ( \
                                   newer.created_at = current_row.created_at \
                               AND newer.id > current_row.id \
                               ) \
                         ) \
                   );",
            )
            .await?;
        }
        DatabaseBackend::MySql => {
            db.execute_unprepared(
                "DELETE current_row \
                 FROM contact_verification_tokens AS current_row \
                 WHERE current_row.consumed_at IS NULL \
                   AND EXISTS ( \
                       SELECT 1 \
                       FROM ( \
                           SELECT newer.id \
                           FROM contact_verification_tokens AS newer \
                           WHERE newer.user_id = current_row.user_id \
                             AND newer.channel = current_row.channel \
                             AND newer.purpose = current_row.purpose \
                             AND newer.consumed_at IS NULL \
                             AND ( \
                                   newer.created_at > current_row.created_at \
                                OR ( \
                                       newer.created_at = current_row.created_at \
                                   AND newer.id > current_row.id \
                                   ) \
                             ) \
                           LIMIT 1 \
                       ) AS dup \
                   );",
            )
            .await?;
        }
        _ => {
            return Err(DbErr::Migration(
                "unsupported database backend for contact verification cleanup".to_string(),
            ));
        }
    }

    Ok(())
}
