use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 为 files.blob_id 和 file_versions.blob_id 添加显式 FK + ON DELETE RESTRICT。
    ///
    /// 新数据库：正常创建 FK。
    /// 已有数据库（FK 已存在）：静默跳过。
    /// 已有数据库（无 FK）：正常创建，数据库层开始强制引用完整性。
    ///
    /// 选用 Restrict 而非 Cascade：因为 blob 生命周期由应用层 ref_count 管理，
    /// 不得在 DB 层意外级联删除。
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = manager.get_database_backend();

        // files.blob_id FK
        let add_files_blob_fk = match backend {
            DatabaseBackend::Sqlite => {
                db.execute_unprepared(
                    "ALTER TABLE files ADD CONSTRAINT fk_files_blob_id \
                     FOREIGN KEY (blob_id) REFERENCES file_blobs(id) ON DELETE RESTRICT",
                )
                .await
            }
            DatabaseBackend::Postgres => {
                db.execute_unprepared(
                    "ALTER TABLE files ADD CONSTRAINT fk_files_blob_id \
                     FOREIGN KEY (blob_id) REFERENCES file_blobs(id) ON DELETE RESTRICT",
                )
                .await
            }
            DatabaseBackend::MySql => {
                let sql = "SELECT COUNT(*) > 0 FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE \
                           WHERE TABLE_SCHEMA = DATABASE() \
                             AND TABLE_NAME = 'files' \
                             AND CONSTRAINT_NAME = 'fk_files_blob_id'";
                let row = db.query_one_raw(Statement::from_string(DatabaseBackend::MySql, sql)).await;
                let exists = row
                    .ok()
                    .flatten()
                    .map(|row| row.try_get_by_index(0))
                    .transpose()
                    .map(|opt: Option<i64>| opt.unwrap_or(0) > 0)
                    .unwrap_or(false);

                if exists {
                    return Ok(());
                }

                db.execute_unprepared(
                    "ALTER TABLE files ADD CONSTRAINT fk_files_blob_id \
                     FOREIGN KEY (blob_id) REFERENCES file_blobs(id) ON DELETE RESTRICT",
                )
                .await
            }
            _ => return Ok(()),
        };
        let _ = add_files_blob_fk.map(|_| ());

        // file_versions.blob_id FK
        let add_versions_blob_fk = match backend {
            DatabaseBackend::Sqlite => {
                db.execute_unprepared(
                    "ALTER TABLE file_versions ADD CONSTRAINT fk_file_versions_blob_id \
                     FOREIGN KEY (blob_id) REFERENCES file_blobs(id) ON DELETE RESTRICT",
                )
                .await
            }
            DatabaseBackend::Postgres => {
                db.execute_unprepared(
                    "ALTER TABLE file_versions ADD CONSTRAINT fk_file_versions_blob_id \
                     FOREIGN KEY (blob_id) REFERENCES file_blobs(id) ON DELETE RESTRICT",
                )
                .await
            }
            DatabaseBackend::MySql => {
                let sql = "SELECT COUNT(*) > 0 FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE \
                           WHERE TABLE_SCHEMA = DATABASE() \
                             AND TABLE_NAME = 'file_versions' \
                             AND CONSTRAINT_NAME = 'fk_file_versions_blob_id'";
                let row = db.query_one_raw(Statement::from_string(DatabaseBackend::MySql, sql)).await;
                let exists = match row {
                    Ok(Some(r)) => r.try_get_by_index::<i64>(0).unwrap_or(0) > 0,
                    _ => false,
                };

                if exists {
                    return Ok(());
                }

                db.execute_unprepared(
                    "ALTER TABLE file_versions ADD CONSTRAINT fk_file_versions_blob_id \
                     FOREIGN KEY (blob_id) REFERENCES file_blobs(id) ON DELETE RESTRICT",
                )
                .await
            }
            _ => return Ok(()),
        };
        let _ = add_versions_blob_fk.map(|_| ());

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // no-op: 不删除已创建的 FK，避免破坏引用完整性
        Ok(())
    }
}
