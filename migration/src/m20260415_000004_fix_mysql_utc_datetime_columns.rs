//! 数据库迁移：`fix_mysql_utc_datetime_columns`。

use sea_orm::{ConnectionTrait, DbBackend, DbErr, Statement};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

struct MysqlUtcDateTimeColumnSpec {
    column: &'static str,
    nullable: bool,
}

struct MysqlUtcDateTimeTableSpec {
    table: &'static str,
    columns: &'static [MysqlUtcDateTimeColumnSpec],
}

const MYSQL_UTC_DATETIME_TABLES: &[MysqlUtcDateTimeTableSpec] = &[
    MysqlUtcDateTimeTableSpec {
        table: "users",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "email_verified_at",
                nullable: true,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "storage_policies",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "user_storage_policies",
        columns: &[MysqlUtcDateTimeColumnSpec {
            column: "created_at",
            nullable: false,
        }],
    },
    MysqlUtcDateTimeTableSpec {
        table: "folders",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "deleted_at",
                nullable: true,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "file_blobs",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "files",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "deleted_at",
                nullable: true,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "system_config",
        columns: &[MysqlUtcDateTimeColumnSpec {
            column: "updated_at",
            nullable: false,
        }],
    },
    MysqlUtcDateTimeTableSpec {
        table: "shares",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "expires_at",
                nullable: true,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "upload_sessions",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "expires_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "webdav_accounts",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "webdav_locks",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "timeout_at",
                nullable: true,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "file_versions",
        columns: &[MysqlUtcDateTimeColumnSpec {
            column: "created_at",
            nullable: false,
        }],
    },
    MysqlUtcDateTimeTableSpec {
        table: "resource_locks",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "timeout_at",
                nullable: true,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "audit_logs",
        columns: &[MysqlUtcDateTimeColumnSpec {
            column: "created_at",
            nullable: false,
        }],
    },
    MysqlUtcDateTimeTableSpec {
        table: "user_profiles",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "upload_session_parts",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "storage_policy_groups",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "storage_policy_group_items",
        columns: &[MysqlUtcDateTimeColumnSpec {
            column: "created_at",
            nullable: false,
        }],
    },
    MysqlUtcDateTimeTableSpec {
        table: "teams",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "archived_at",
                nullable: true,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "team_members",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "contact_verification_tokens",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "expires_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "consumed_at",
                nullable: true,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "mail_outbox",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "next_attempt_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "processing_started_at",
                nullable: true,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "sent_at",
                nullable: true,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "background_tasks",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "next_run_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "processing_started_at",
                nullable: true,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "started_at",
                nullable: true,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "finished_at",
                nullable: true,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "expires_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "updated_at",
                nullable: false,
            },
        ],
    },
    MysqlUtcDateTimeTableSpec {
        table: "wopi_sessions",
        columns: &[
            MysqlUtcDateTimeColumnSpec {
                column: "expires_at",
                nullable: false,
            },
            MysqlUtcDateTimeColumnSpec {
                column: "created_at",
                nullable: false,
            },
        ],
    },
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        apply_mysql_datetime_fix(manager, crate::time::mysql_datetime_definition).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        apply_mysql_datetime_fix(manager, crate::time::mysql_timestamp_definition).await
    }
}

async fn apply_mysql_datetime_fix(
    manager: &SchemaManager<'_>,
    column_definition: fn(bool) -> &'static str,
) -> Result<(), DbErr> {
    if manager.get_database_backend() != DatabaseBackend::MySql {
        return Ok(());
    }

    let db = manager.get_connection();

    for table in MYSQL_UTC_DATETIME_TABLES {
        if !mysql_table_exists(manager, table.table).await? {
            continue;
        }

        let clauses = table
            .columns
            .iter()
            .map(|column| {
                format!(
                    "MODIFY COLUMN `{}` {}",
                    column.column,
                    column_definition(column.nullable)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        db.execute_unprepared(&format!("ALTER TABLE `{}` {clauses}", table.table))
            .await?;
    }

    Ok(())
}

async fn mysql_table_exists(manager: &SchemaManager<'_>, table_name: &str) -> Result<bool, DbErr> {
    let sql = format!(
        "SELECT 1 FROM information_schema.tables \
         WHERE table_schema = DATABASE() \
           AND table_name = '{}' \
         LIMIT 1",
        table_name.replace('\'', "''")
    );

    manager
        .get_connection()
        .query_one_raw(Statement::from_string(DbBackend::MySql, sql))
        .await
        .map(|row| row.is_some())
}
