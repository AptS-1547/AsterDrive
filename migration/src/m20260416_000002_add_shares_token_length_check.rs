//! 数据库迁移：`add_shares_token_length_check`。

use sea_orm::{ConnectionTrait, DatabaseBackend, DbErr, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const SHARE_TOKEN_LENGTH_CHECK_NAME: &str = "chk_shares_token_length_32";

const SQLITE_REBUILD_WITH_TOKEN_LENGTH_CHECK: &str = r#"
CREATE TABLE "shares__new" (
    "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
    "token" varchar(32) NOT NULL UNIQUE,
    "user_id" integer NOT NULL,
    "file_id" integer NULL,
    "folder_id" integer NULL,
    "password" varchar(255) NULL,
    "expires_at" timestamp_with_timezone_text NULL,
    "max_downloads" integer NOT NULL DEFAULT 0,
    "download_count" integer NOT NULL DEFAULT 0,
    "view_count" integer NOT NULL DEFAULT 0,
    "created_at" timestamp_with_timezone_text NOT NULL,
    "updated_at" timestamp_with_timezone_text NOT NULL,
    "team_id" integer NULL,
    CONSTRAINT "chk_shares_exactly_one_target" CHECK (("file_id" IS NULL) <> ("folder_id" IS NULL)),
    CONSTRAINT "chk_shares_token_length_32" CHECK (length("token") <= 32),
    FOREIGN KEY ("user_id") REFERENCES "users" ("id") ON DELETE CASCADE
);
INSERT INTO "shares__new" (
    "id",
    "token",
    "user_id",
    "file_id",
    "folder_id",
    "password",
    "expires_at",
    "max_downloads",
    "download_count",
    "view_count",
    "created_at",
    "updated_at",
    "team_id"
)
SELECT
    "id",
    "token",
    "user_id",
    "file_id",
    "folder_id",
    "password",
    "expires_at",
    "max_downloads",
    "download_count",
    "view_count",
    "created_at",
    "updated_at",
    "team_id"
FROM "shares";
DROP TABLE "shares";
ALTER TABLE "shares__new" RENAME TO "shares";
CREATE UNIQUE INDEX "idx_shares_token" ON "shares" ("token");
CREATE INDEX "idx_shares_user_file" ON "shares" ("user_id", "file_id");
CREATE INDEX "idx_shares_user_folder" ON "shares" ("user_id", "folder_id");
CREATE INDEX "idx_shares_team_id" ON "shares" ("team_id");
CREATE INDEX "idx_shares_team_file" ON "shares" ("team_id", "file_id");
CREATE INDEX "idx_shares_team_folder" ON "shares" ("team_id", "folder_id");
"#;

const SQLITE_REBUILD_WITHOUT_TOKEN_LENGTH_CHECK: &str = r#"
CREATE TABLE "shares__new" (
    "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
    "token" varchar(32) NOT NULL UNIQUE,
    "user_id" integer NOT NULL,
    "file_id" integer NULL,
    "folder_id" integer NULL,
    "password" varchar(255) NULL,
    "expires_at" timestamp_with_timezone_text NULL,
    "max_downloads" integer NOT NULL DEFAULT 0,
    "download_count" integer NOT NULL DEFAULT 0,
    "view_count" integer NOT NULL DEFAULT 0,
    "created_at" timestamp_with_timezone_text NOT NULL,
    "updated_at" timestamp_with_timezone_text NOT NULL,
    "team_id" integer NULL,
    CONSTRAINT "chk_shares_exactly_one_target" CHECK (("file_id" IS NULL) <> ("folder_id" IS NULL)),
    FOREIGN KEY ("user_id") REFERENCES "users" ("id") ON DELETE CASCADE
);
INSERT INTO "shares__new" (
    "id",
    "token",
    "user_id",
    "file_id",
    "folder_id",
    "password",
    "expires_at",
    "max_downloads",
    "download_count",
    "view_count",
    "created_at",
    "updated_at",
    "team_id"
)
SELECT
    "id",
    "token",
    "user_id",
    "file_id",
    "folder_id",
    "password",
    "expires_at",
    "max_downloads",
    "download_count",
    "view_count",
    "created_at",
    "updated_at",
    "team_id"
FROM "shares";
DROP TABLE "shares";
ALTER TABLE "shares__new" RENAME TO "shares";
CREATE UNIQUE INDEX "idx_shares_token" ON "shares" ("token");
CREATE INDEX "idx_shares_user_file" ON "shares" ("user_id", "file_id");
CREATE INDEX "idx_shares_user_folder" ON "shares" ("user_id", "folder_id");
CREATE INDEX "idx_shares_team_id" ON "shares" ("team_id");
CREATE INDEX "idx_shares_team_file" ON "shares" ("team_id", "file_id");
CREATE INDEX "idx_shares_team_folder" ON "shares" ("team_id", "folder_id");
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        ensure_valid_share_token_lengths(manager).await?;

        let db = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Sqlite => {
                db.execute_unprepared(SQLITE_REBUILD_WITH_TOKEN_LENGTH_CHECK)
                    .await
            }
            DatabaseBackend::Postgres => {
                db.execute_unprepared(&format!(
                    "ALTER TABLE shares ADD CONSTRAINT {SHARE_TOKEN_LENGTH_CHECK_NAME} \
                     CHECK (char_length(token) <= 32);"
                ))
                .await
            }
            DatabaseBackend::MySql => {
                db.execute_unprepared(&format!(
                    "ALTER TABLE shares ADD CONSTRAINT {SHARE_TOKEN_LENGTH_CHECK_NAME} \
                     CHECK (char_length(token) <= 32);"
                ))
                .await
            }
            _ => Err(DbErr::Migration(
                "unsupported database backend for shares token length check".to_string(),
            )),
        }?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Sqlite => {
                db.execute_unprepared(SQLITE_REBUILD_WITHOUT_TOKEN_LENGTH_CHECK)
                    .await
            }
            DatabaseBackend::Postgres => {
                db.execute_unprepared(&format!(
                    "ALTER TABLE shares DROP CONSTRAINT IF EXISTS {SHARE_TOKEN_LENGTH_CHECK_NAME};"
                ))
                .await
            }
            DatabaseBackend::MySql => {
                db.execute_unprepared(&format!(
                    "ALTER TABLE shares DROP CHECK {SHARE_TOKEN_LENGTH_CHECK_NAME};"
                ))
                .await
            }
            _ => Err(DbErr::Migration(
                "unsupported database backend for shares token length check".to_string(),
            )),
        }?;

        Ok(())
    }
}

async fn ensure_valid_share_token_lengths(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let sql = match manager.get_database_backend() {
        DatabaseBackend::Sqlite => "SELECT COUNT(*) FROM shares WHERE length(token) > 32",
        DatabaseBackend::Postgres | DatabaseBackend::MySql => {
            "SELECT COUNT(*) FROM shares WHERE char_length(token) > 32"
        }
        _ => {
            return Err(DbErr::Migration(
                "unsupported database backend for shares token length check".to_string(),
            ));
        }
    };

    let db = manager.get_connection();
    let row = db
        .query_one_raw(Statement::from_string(manager.get_database_backend(), sql))
        .await?;

    let Some(row) = row else {
        return Ok(());
    };

    let invalid_count: i64 = row.try_get_by_index(0).map_err(|error| {
        DbErr::Custom(format!("read invalid shares token length count: {error}"))
    })?;

    if invalid_count > 0 {
        return Err(DbErr::Migration(format!(
            "cannot add {SHARE_TOKEN_LENGTH_CHECK_NAME}: found {invalid_count} share row(s) with token length > 32"
        )));
    }

    Ok(())
}
