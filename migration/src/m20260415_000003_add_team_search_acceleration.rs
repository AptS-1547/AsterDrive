use crate::search_acceleration::{
    SqliteFtsConfig, ensure_postgres_extension, execute_sqlite_statements,
    mysql_fulltext_index_sql, postgres_drop_index, postgres_trigram_index,
    sqlite_fts_down_statements, sqlite_fts_up_statements,
};
use sea_orm::{ConnectionTrait, DbBackend, DbErr};
use sea_orm_migration::prelude::*;

const SQLITE_TEAMS_FTS_TABLE: &str = "teams_search_fts";
const SQLITE_TEAMS_INSERT_TRIGGER: &str = "trg_teams_search_fts_ai";
const SQLITE_TEAMS_DELETE_TRIGGER: &str = "trg_teams_search_fts_ad";
const SQLITE_TEAMS_UPDATE_TRIGGER: &str = "trg_teams_search_fts_au";
const POSTGRES_TEAMS_NAME_TRGM_INDEX: &str = "idx_teams_name_trgm";
const POSTGRES_TEAMS_DESCRIPTION_TRGM_INDEX: &str = "idx_teams_description_trgm";
const MYSQL_TEAMS_SEARCH_FULLTEXT_INDEX: &str = "idx_teams_search_fulltext";

#[derive(DeriveMigrationName)]
pub struct Migration;

fn sqlite_up_statements() -> Vec<String> {
    sqlite_fts_up_statements(&SqliteFtsConfig {
        virtual_table: SQLITE_TEAMS_FTS_TABLE,
        source_table: "teams",
        columns: &["name", "description"],
        insert_trigger: SQLITE_TEAMS_INSERT_TRIGGER,
        delete_trigger: SQLITE_TEAMS_DELETE_TRIGGER,
        update_trigger: SQLITE_TEAMS_UPDATE_TRIGGER,
    })
}

fn sqlite_down_statements() -> Vec<String> {
    sqlite_fts_down_statements(&SqliteFtsConfig {
        virtual_table: SQLITE_TEAMS_FTS_TABLE,
        source_table: "teams",
        columns: &["name", "description"],
        insert_trigger: SQLITE_TEAMS_INSERT_TRIGGER,
        delete_trigger: SQLITE_TEAMS_DELETE_TRIGGER,
        update_trigger: SQLITE_TEAMS_UPDATE_TRIGGER,
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
                        POSTGRES_TEAMS_NAME_TRGM_INDEX,
                        "teams",
                        "name",
                    ))
                    .await?;
                manager
                    .create_index(postgres_trigram_index(
                        POSTGRES_TEAMS_DESCRIPTION_TRGM_INDEX,
                        "teams",
                        "description",
                    ))
                    .await?;
                Ok(())
            }
            DbBackend::MySql => {
                db.execute_unprepared(&mysql_fulltext_index_sql(
                    MYSQL_TEAMS_SEARCH_FULLTEXT_INDEX,
                    "teams",
                    &["name", "description"],
                ))
                .await?;
                Ok(())
            }
            DbBackend::Sqlite => execute_sqlite_statements(
                manager,
                sqlite_up_statements(),
                "SQLite team search acceleration migration requires FTS5 with trigram tokenizer support",
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
                    .drop_index(postgres_drop_index(POSTGRES_TEAMS_NAME_TRGM_INDEX))
                    .await?;
                manager
                    .drop_index(postgres_drop_index(POSTGRES_TEAMS_DESCRIPTION_TRGM_INDEX))
                    .await?;
                Ok(())
            }
            DbBackend::MySql => {
                manager
                    .drop_index(
                        Index::drop()
                            .name(MYSQL_TEAMS_SEARCH_FULLTEXT_INDEX)
                            .table(Alias::new("teams"))
                            .to_owned(),
                    )
                    .await?;
                Ok(())
            }
            DbBackend::Sqlite => execute_sqlite_statements(
                manager,
                sqlite_down_statements(),
                "SQLite team search acceleration migration requires FTS5 with trigram tokenizer support",
            )
            .await,
            _ => Ok(()),
        }
    }
}
