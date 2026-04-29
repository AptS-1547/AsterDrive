//! 数据库迁移：`drop_master_binding_ingress_policy_id`。

use sea_orm::{DbBackend, DbErr, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const SQLITE_REBUILD_WITHOUT_INGRESS_POLICY_ID: &str = r#"
CREATE TABLE "master_bindings__new" (
    "id" integer NOT NULL PRIMARY KEY,
    "name" varchar(128) NOT NULL,
    "master_url" varchar(512) NOT NULL,
    "access_key" varchar(128) NOT NULL,
    "secret_key" varchar(255) NOT NULL,
    "namespace" varchar(128) NOT NULL,
    "is_enabled" boolean NOT NULL DEFAULT 1,
    "created_at" timestamp_with_timezone_text NOT NULL,
    "updated_at" timestamp_with_timezone_text NOT NULL
);
INSERT INTO "master_bindings__new" (
    "id",
    "name",
    "master_url",
    "access_key",
    "secret_key",
    "namespace",
    "is_enabled",
    "created_at",
    "updated_at"
)
SELECT
    "id",
    "name",
    "master_url",
    "access_key",
    "secret_key",
    "namespace",
    "is_enabled",
    "created_at",
    "updated_at"
FROM "master_bindings";
DROP TABLE "master_bindings";
ALTER TABLE "master_bindings__new" RENAME TO "master_bindings";
CREATE UNIQUE INDEX "idx_master_bindings_access_key" ON "master_bindings" ("access_key");
"#;

const SQLITE_REBUILD_WITH_INGRESS_POLICY_ID: &str = r#"
CREATE TABLE "master_bindings__new" (
    "id" integer NOT NULL PRIMARY KEY,
    "name" varchar(128) NOT NULL,
    "master_url" varchar(512) NOT NULL,
    "access_key" varchar(128) NOT NULL,
    "secret_key" varchar(255) NOT NULL,
    "namespace" varchar(128) NOT NULL,
    "ingress_policy_id" integer NULL,
    "is_enabled" boolean NOT NULL DEFAULT 1,
    "created_at" timestamp_with_timezone_text NOT NULL,
    "updated_at" timestamp_with_timezone_text NOT NULL,
    FOREIGN KEY ("ingress_policy_id") REFERENCES "storage_policies" ("id") ON DELETE RESTRICT
);
INSERT INTO "master_bindings__new" (
    "id",
    "name",
    "master_url",
    "access_key",
    "secret_key",
    "namespace",
    "ingress_policy_id",
    "is_enabled",
    "created_at",
    "updated_at"
)
SELECT
    "id",
    "name",
    "master_url",
    "access_key",
    "secret_key",
    "namespace",
    NULL,
    "is_enabled",
    "created_at",
    "updated_at"
FROM "master_bindings";
DROP TABLE "master_bindings";
ALTER TABLE "master_bindings__new" RENAME TO "master_bindings";
CREATE UNIQUE INDEX "idx_master_bindings_access_key" ON "master_bindings" ("access_key");
CREATE INDEX "idx_master_bindings_ingress_policy_id" ON "master_bindings" ("ingress_policy_id");
"#;

const MASTER_BINDINGS_INGRESS_POLICY_FK: &str = "fk_master_bindings_ingress_policy_id";
const MASTER_BINDINGS_INGRESS_POLICY_INDEX: &str = "idx_master_bindings_ingress_policy_id";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match db.get_database_backend() {
            DbBackend::Sqlite => {
                db.execute_unprepared(SQLITE_REBUILD_WITHOUT_INGRESS_POLICY_ID)
                    .await?;
                Ok(())
            }
            DbBackend::MySql => {
                drop_mysql_ingress_policy_foreign_key(manager).await?;
                drop_index_if_exists_mysql(db, MASTER_BINDINGS_INGRESS_POLICY_INDEX).await?;
                db.execute_unprepared("ALTER TABLE master_bindings DROP COLUMN ingress_policy_id")
                    .await?;
                Ok(())
            }
            DbBackend::Postgres => {
                db.execute_unprepared("ALTER TABLE master_bindings DROP COLUMN ingress_policy_id")
                    .await?;
                Ok(())
            }
            backend => Err(DbErr::Migration(format!(
                "unsupported database backend for dropping master_bindings ingress_policy_id: {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match db.get_database_backend() {
            DbBackend::Sqlite => {
                db.execute_unprepared(SQLITE_REBUILD_WITH_INGRESS_POLICY_ID)
                    .await?;
                Ok(())
            }
            DbBackend::MySql => {
                db.execute_unprepared(
                    "ALTER TABLE master_bindings ADD COLUMN ingress_policy_id BIGINT NULL",
                )
                .await?;
                manager
                    .create_index(
                        Index::create()
                            .name(MASTER_BINDINGS_INGRESS_POLICY_INDEX)
                            .table(MasterBindings::Table)
                            .col(MasterBindings::IngressPolicyId)
                            .to_owned(),
                    )
                    .await?;
                manager
                    .create_foreign_key(
                        ForeignKey::create()
                            .name(MASTER_BINDINGS_INGRESS_POLICY_FK)
                            .from(MasterBindings::Table, MasterBindings::IngressPolicyId)
                            .to(StoragePolicies::Table, StoragePolicies::Id)
                            .on_delete(ForeignKeyAction::Restrict)
                            .to_owned(),
                    )
                    .await
            }
            DbBackend::Postgres => {
                db.execute_unprepared(
                    "ALTER TABLE master_bindings ADD COLUMN ingress_policy_id BIGINT NULL",
                )
                .await?;
                manager
                    .create_index(
                        Index::create()
                            .name(MASTER_BINDINGS_INGRESS_POLICY_INDEX)
                            .table(MasterBindings::Table)
                            .col(MasterBindings::IngressPolicyId)
                            .to_owned(),
                    )
                    .await?;
                manager
                    .create_foreign_key(
                        ForeignKey::create()
                            .name(MASTER_BINDINGS_INGRESS_POLICY_FK)
                            .from(MasterBindings::Table, MasterBindings::IngressPolicyId)
                            .to(StoragePolicies::Table, StoragePolicies::Id)
                            .on_delete(ForeignKeyAction::Restrict)
                            .to_owned(),
                    )
                    .await
            }
            backend => Err(DbErr::Migration(format!(
                "unsupported database backend for restoring master_bindings ingress_policy_id: {backend:?}"
            ))),
        }
    }
}

async fn drop_mysql_ingress_policy_foreign_key(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::MySql,
            "SELECT CONSTRAINT_NAME \
             FROM information_schema.KEY_COLUMN_USAGE \
             WHERE TABLE_SCHEMA = DATABASE() \
               AND TABLE_NAME = 'master_bindings' \
               AND COLUMN_NAME = 'ingress_policy_id' \
               AND REFERENCED_TABLE_NAME IS NOT NULL \
             LIMIT 1",
        ))
        .await?;

    let Some(row) = row else {
        return Ok(());
    };

    let constraint_name: String = row.try_get_by_index(0).map_err(|e| {
        DbErr::Custom(format!(
            "read master_bindings ingress_policy_id foreign key: {e}"
        ))
    })?;

    db.execute_unprepared(&format!(
        "ALTER TABLE master_bindings DROP FOREIGN KEY `{constraint_name}`"
    ))
    .await?;

    Ok(())
}

async fn drop_index_if_exists_mysql<C: sea_orm::ConnectionTrait>(
    db: &C,
    index_name: &str,
) -> Result<(), DbErr> {
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::MySql,
            format!(
                "SELECT INDEX_NAME \
                 FROM information_schema.statistics \
                 WHERE TABLE_SCHEMA = DATABASE() \
                   AND TABLE_NAME = 'master_bindings' \
                   AND INDEX_NAME = '{}' \
                 LIMIT 1",
                index_name
            ),
        ))
        .await?;

    if row.is_none() {
        return Ok(());
    }

    db.execute_unprepared(&format!(
        "ALTER TABLE master_bindings DROP INDEX `{index_name}`"
    ))
    .await?;
    Ok(())
}

#[derive(DeriveIden)]
enum MasterBindings {
    Table,
    IngressPolicyId,
}

#[derive(DeriveIden)]
enum StoragePolicies {
    Table,
    Id,
}
