use crate::search_acceleration::{
    SqliteFtsConfig, ensure_postgres_extension, execute_sqlite_statements,
    mysql_fulltext_index_sql, postgres_drop_index, postgres_trigram_index,
    sqlite_fts_down_statements, sqlite_fts_up_statements,
};
use sea_orm::{ConnectionTrait, DbBackend, DbErr};
use sea_orm_migration::prelude::*;

const SQLITE_USERS_FTS_TABLE: &str = "users_search_fts";
const SQLITE_USERS_INSERT_TRIGGER: &str = "trg_users_search_fts_ai";
const SQLITE_USERS_DELETE_TRIGGER: &str = "trg_users_search_fts_ad";
const SQLITE_USERS_UPDATE_TRIGGER: &str = "trg_users_search_fts_au";
const POSTGRES_USERNAME_TRGM_INDEX: &str = "idx_users_username_trgm";
const POSTGRES_EMAIL_TRGM_INDEX: &str = "idx_users_email_trgm";
const MYSQL_USERS_SEARCH_FULLTEXT_INDEX: &str = "idx_users_search_fulltext";

#[derive(DeriveMigrationName)]
pub struct Migration;

fn sqlite_up_statements() -> Vec<String> {
    sqlite_fts_up_statements(&SqliteFtsConfig {
        virtual_table: SQLITE_USERS_FTS_TABLE,
        source_table: "users",
        columns: &["username", "email"],
        insert_trigger: SQLITE_USERS_INSERT_TRIGGER,
        delete_trigger: SQLITE_USERS_DELETE_TRIGGER,
        update_trigger: SQLITE_USERS_UPDATE_TRIGGER,
    })
}

fn sqlite_down_statements() -> Vec<String> {
    sqlite_fts_down_statements(&SqliteFtsConfig {
        virtual_table: SQLITE_USERS_FTS_TABLE,
        source_table: "users",
        columns: &["username", "email"],
        insert_trigger: SQLITE_USERS_INSERT_TRIGGER,
        delete_trigger: SQLITE_USERS_DELETE_TRIGGER,
        update_trigger: SQLITE_USERS_UPDATE_TRIGGER,
    })
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        match db.get_database_backend() {
            DbBackend::Postgres => {
                ensure_postgres_extension(manager, "pg_trgm").await?;
                manager
                    .create_index(postgres_trigram_index(
                        POSTGRES_USERNAME_TRGM_INDEX,
                        "users",
                        "username",
                    ))
                    .await?;
                manager
                    .create_index(postgres_trigram_index(
                        POSTGRES_EMAIL_TRGM_INDEX,
                        "users",
                        "email",
                    ))
                    .await?;
                Ok(())
            }
            DbBackend::MySql => {
                db.execute_unprepared(&mysql_fulltext_index_sql(
                    MYSQL_USERS_SEARCH_FULLTEXT_INDEX,
                    "users",
                    &["username", "email"],
                ))
                .await?;
                Ok(())
            }
            DbBackend::Sqlite => execute_sqlite_statements(
                manager,
                sqlite_up_statements(),
                "SQLite user search acceleration migration requires FTS5 with trigram tokenizer support",
            )
            .await,
            _ => Ok(()),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        match db.get_database_backend() {
            DbBackend::Postgres => {
                manager
                    .drop_index(postgres_drop_index(POSTGRES_USERNAME_TRGM_INDEX))
                    .await?;
                manager
                    .drop_index(postgres_drop_index(POSTGRES_EMAIL_TRGM_INDEX))
                    .await?;
                Ok(())
            }
            DbBackend::MySql => {
                manager
                    .drop_index(
                        Index::drop()
                            .name(MYSQL_USERS_SEARCH_FULLTEXT_INDEX)
                            .table(Alias::new("users"))
                            .to_owned(),
                    )
                    .await?;
                Ok(())
            }
            DbBackend::Sqlite => execute_sqlite_statements(
                manager,
                sqlite_down_statements(),
                "SQLite user search acceleration migration requires FTS5 with trigram tokenizer support",
            )
            .await,
            _ => Ok(()),
        }
    }
}
