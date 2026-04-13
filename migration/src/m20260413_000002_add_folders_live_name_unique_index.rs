use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        match manager.get_database_backend() {
            DatabaseBackend::Sqlite => {
                db.execute_unprepared(
                    "CREATE UNIQUE INDEX idx_folders_unique_live_name \
                     ON folders ( \
                        (CASE WHEN team_id IS NULL THEN 0 ELSE 1 END), \
                        (CASE WHEN team_id IS NULL THEN user_id ELSE team_id END), \
                        (COALESCE(parent_id, 0)), \
                        name, \
                        (CASE WHEN deleted_at IS NULL THEN 1 ELSE NULL END) \
                     );",
                )
                .await?;
            }
            DatabaseBackend::Postgres => {
                db.execute_unprepared(
                    "CREATE UNIQUE INDEX idx_folders_unique_live_name \
                     ON folders ( \
                        (CASE WHEN team_id IS NULL THEN 0 ELSE 1 END), \
                        (CASE WHEN team_id IS NULL THEN user_id ELSE team_id END), \
                        (COALESCE(parent_id, 0)), \
                        name, \
                        (CASE WHEN deleted_at IS NULL THEN 1 ELSE NULL END) \
                     );",
                )
                .await?;
            }
            DatabaseBackend::MySql => {
                db.execute_unprepared(
                    "CREATE UNIQUE INDEX idx_folders_unique_live_name \
                     ON folders ( \
                        ((CASE WHEN team_id IS NULL THEN 0 ELSE 1 END)), \
                        ((CASE WHEN team_id IS NULL THEN user_id ELSE team_id END)), \
                        ((COALESCE(parent_id, 0))), \
                        name, \
                        ((CASE WHEN deleted_at IS NULL THEN 1 ELSE NULL END)) \
                     );",
                )
                .await?;
            }
            _ => {
                return Err(DbErr::Migration(
                    "unsupported database backend for folders live-name unique index".to_string(),
                ));
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        match manager.get_database_backend() {
            DatabaseBackend::Sqlite | DatabaseBackend::Postgres => {
                db.execute_unprepared("DROP INDEX IF EXISTS idx_folders_unique_live_name;")
                    .await?;
            }
            DatabaseBackend::MySql => {
                db.execute_unprepared("DROP INDEX idx_folders_unique_live_name ON folders;")
                    .await?;
            }
            _ => {
                return Err(DbErr::Migration(
                    "unsupported database backend for folders live-name unique index".to_string(),
                ));
            }
        }

        Ok(())
    }
}
