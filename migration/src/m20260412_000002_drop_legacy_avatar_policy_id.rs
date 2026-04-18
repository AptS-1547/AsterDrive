//! 数据库迁移：`drop_legacy_avatar_policy_id`。

use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const SQLITE_REBUILD_WITHOUT_AVATAR_POLICY_ID: &str = r#"
CREATE TABLE "user_profiles__new" (
    "user_id" integer NOT NULL PRIMARY KEY,
    "avatar_source" varchar(16) NOT NULL DEFAULT 'none',
    "avatar_key" varchar(512) NULL,
    "avatar_version" integer NOT NULL DEFAULT 0,
    "created_at" timestamp_with_timezone_text NOT NULL,
    "updated_at" timestamp_with_timezone_text NOT NULL,
    "display_name" varchar(64) NULL,
    FOREIGN KEY ("user_id") REFERENCES "users" ("id") ON DELETE CASCADE
);
INSERT INTO "user_profiles__new" (
    "user_id",
    "avatar_source",
    "avatar_key",
    "avatar_version",
    "created_at",
    "updated_at",
    "display_name"
)
SELECT
    "user_id",
    "avatar_source",
    "avatar_key",
    "avatar_version",
    "created_at",
    "updated_at",
    "display_name"
FROM "user_profiles";
DROP TABLE "user_profiles";
ALTER TABLE "user_profiles__new" RENAME TO "user_profiles";
"#;

const SQLITE_REBUILD_WITH_AVATAR_POLICY_ID: &str = r#"
CREATE TABLE "user_profiles__new" (
    "user_id" integer NOT NULL PRIMARY KEY,
    "avatar_source" varchar(16) NOT NULL DEFAULT 'none',
    "avatar_policy_id" integer NULL,
    "avatar_key" varchar(512) NULL,
    "avatar_version" integer NOT NULL DEFAULT 0,
    "created_at" timestamp_with_timezone_text NOT NULL,
    "updated_at" timestamp_with_timezone_text NOT NULL,
    "display_name" varchar(64) NULL,
    FOREIGN KEY ("user_id") REFERENCES "users" ("id") ON DELETE CASCADE,
    FOREIGN KEY ("avatar_policy_id") REFERENCES "storage_policies" ("id") ON DELETE SET NULL
);
INSERT INTO "user_profiles__new" (
    "user_id",
    "avatar_source",
    "avatar_policy_id",
    "avatar_key",
    "avatar_version",
    "created_at",
    "updated_at",
    "display_name"
)
SELECT
    "user_id",
    "avatar_source",
    NULL,
    "avatar_key",
    "avatar_version",
    "created_at",
    "updated_at",
    "display_name"
FROM "user_profiles";
DROP TABLE "user_profiles";
ALTER TABLE "user_profiles__new" RENAME TO "user_profiles";
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        if db.get_database_backend() == DbBackend::Sqlite {
            db.execute_unprepared(SQLITE_REBUILD_WITHOUT_AVATAR_POLICY_ID)
                .await?;
            return Ok(());
        }

        if db.get_database_backend() == DbBackend::MySql {
            drop_mysql_avatar_policy_foreign_key(manager).await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(UserProfiles::Table)
                    .drop_column(UserProfiles::AvatarPolicyId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        if db.get_database_backend() == DbBackend::Sqlite {
            db.execute_unprepared(SQLITE_REBUILD_WITH_AVATAR_POLICY_ID)
                .await?;
            return Ok(());
        }

        manager
            .alter_table(
                Table::alter()
                    .table(UserProfiles::Table)
                    .add_column(
                        ColumnDef::new(UserProfiles::AvatarPolicyId)
                            .big_integer()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_user_profiles_avatar_policy_id")
                    .from(UserProfiles::Table, UserProfiles::AvatarPolicyId)
                    .to(StoragePolicies::Table, StoragePolicies::Id)
                    .on_delete(ForeignKeyAction::SetNull)
                    .to_owned(),
            )
            .await
    }
}

async fn drop_mysql_avatar_policy_foreign_key(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::MySql,
            "SELECT CONSTRAINT_NAME \
             FROM information_schema.KEY_COLUMN_USAGE \
             WHERE TABLE_SCHEMA = DATABASE() \
               AND TABLE_NAME = 'user_profiles' \
               AND COLUMN_NAME = 'avatar_policy_id' \
               AND REFERENCED_TABLE_NAME IS NOT NULL \
             LIMIT 1",
        ))
        .await?;

    let Some(row) = row else {
        return Ok(());
    };

    let constraint_name: String = row.try_get_by_index(0).map_err(|e| {
        DbErr::Custom(format!(
            "read user_profiles avatar_policy_id foreign key: {e}"
        ))
    })?;

    db.execute_unprepared(&format!(
        "ALTER TABLE user_profiles DROP FOREIGN KEY `{constraint_name}`"
    ))
    .await?;

    Ok(())
}

#[derive(DeriveIden)]
enum UserProfiles {
    Table,
    AvatarPolicyId,
}

#[derive(DeriveIden)]
enum StoragePolicies {
    Table,
    Id,
}
