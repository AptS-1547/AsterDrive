//! 数据库迁移：`add_sqlite_search_fts`。

use crate::search_acceleration::{
    SqliteFtsConfig, execute_sqlite_statements, sqlite_fts_down_statements,
    sqlite_fts_up_statements,
};
use sea_orm::{ConnectionTrait, DbBackend, DbErr};
use sea_orm_migration::prelude::*;

const FILES_FTS_TABLE: &str = "files_name_fts";
const FOLDERS_FTS_TABLE: &str = "folders_name_fts";
const FILES_INSERT_TRIGGER: &str = "trg_files_name_fts_ai";
const FILES_DELETE_TRIGGER: &str = "trg_files_name_fts_ad";
const FILES_UPDATE_TRIGGER: &str = "trg_files_name_fts_au";
const FOLDERS_INSERT_TRIGGER: &str = "trg_folders_name_fts_ai";
const FOLDERS_DELETE_TRIGGER: &str = "trg_folders_name_fts_ad";
const FOLDERS_UPDATE_TRIGGER: &str = "trg_folders_name_fts_au";

#[derive(DeriveMigrationName)]
pub struct Migration;

fn sqlite_up_statements() -> Vec<String> {
    let files = SqliteFtsConfig {
        virtual_table: FILES_FTS_TABLE,
        source_table: "files",
        columns: &["name"],
        insert_trigger: FILES_INSERT_TRIGGER,
        delete_trigger: FILES_DELETE_TRIGGER,
        update_trigger: FILES_UPDATE_TRIGGER,
    };
    let folders = SqliteFtsConfig {
        virtual_table: FOLDERS_FTS_TABLE,
        source_table: "folders",
        columns: &["name"],
        insert_trigger: FOLDERS_INSERT_TRIGGER,
        delete_trigger: FOLDERS_DELETE_TRIGGER,
        update_trigger: FOLDERS_UPDATE_TRIGGER,
    };

    let mut statements = sqlite_fts_up_statements(&files);
    statements.extend(sqlite_fts_up_statements(&folders));
    statements
}

fn sqlite_down_statements() -> Vec<String> {
    let files = SqliteFtsConfig {
        virtual_table: FILES_FTS_TABLE,
        source_table: "files",
        columns: &["name"],
        insert_trigger: FILES_INSERT_TRIGGER,
        delete_trigger: FILES_DELETE_TRIGGER,
        update_trigger: FILES_UPDATE_TRIGGER,
    };
    let folders = SqliteFtsConfig {
        virtual_table: FOLDERS_FTS_TABLE,
        source_table: "folders",
        columns: &["name"],
        insert_trigger: FOLDERS_INSERT_TRIGGER,
        delete_trigger: FOLDERS_DELETE_TRIGGER,
        update_trigger: FOLDERS_UPDATE_TRIGGER,
    };

    let mut statements = sqlite_fts_down_statements(&files);
    statements.extend(sqlite_fts_down_statements(&folders));
    statements
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_connection().get_database_backend() != DbBackend::Sqlite {
            return Ok(());
        }

        execute_sqlite_statements(
            manager,
            sqlite_up_statements(),
            "SQLite search acceleration migration requires FTS5 with trigram tokenizer support",
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_connection().get_database_backend() != DbBackend::Sqlite {
            return Ok(());
        }

        execute_sqlite_statements(
            manager,
            sqlite_down_statements(),
            "SQLite search acceleration migration requires FTS5 with trigram tokenizer support",
        )
        .await
    }
}
