//! 数据库迁移：`add_search_name_indexes`。

use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        match db.get_database_backend() {
            DbBackend::Postgres => {
                db.execute_unprepared("CREATE EXTENSION IF NOT EXISTS pg_trgm;")
                    .await?;
                db.execute_unprepared(
                    "CREATE INDEX IF NOT EXISTS idx_files_live_name_trgm \
                     ON files USING gin (name gin_trgm_ops) \
                     WHERE deleted_at IS NULL",
                )
                .await?;
                db.execute_unprepared(
                    "CREATE INDEX IF NOT EXISTS idx_folders_live_name_trgm \
                     ON folders USING gin (name gin_trgm_ops) \
                     WHERE deleted_at IS NULL",
                )
                .await?;
            }
            DbBackend::MySql => {
                db.execute_unprepared(
                    "CREATE FULLTEXT INDEX idx_files_name_fulltext \
                     ON files (name) WITH PARSER ngram",
                )
                .await?;
                db.execute_unprepared(
                    "CREATE FULLTEXT INDEX idx_folders_name_fulltext \
                     ON folders (name) WITH PARSER ngram",
                )
                .await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        match db.get_database_backend() {
            DbBackend::Postgres => {
                db.execute_unprepared("DROP INDEX IF EXISTS idx_files_live_name_trgm;")
                    .await?;
                db.execute_unprepared("DROP INDEX IF EXISTS idx_folders_live_name_trgm;")
                    .await?;
            }
            DbBackend::MySql => {
                db.execute_unprepared("DROP INDEX idx_files_name_fulltext ON files")
                    .await?;
                db.execute_unprepared("DROP INDEX idx_folders_name_fulltext ON folders")
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}
