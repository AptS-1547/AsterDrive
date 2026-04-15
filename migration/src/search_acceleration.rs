use sea_orm::{ConnectionTrait, DbErr};
use sea_orm_migration::prelude::*;
use sea_query::{Alias, IntoIndexColumn, PostgresQueryBuilder, extension::postgres::Extension};

pub struct SqliteFtsConfig<'a> {
    pub virtual_table: &'a str,
    pub source_table: &'a str,
    pub columns: &'a [&'a str],
    pub insert_trigger: &'a str,
    pub delete_trigger: &'a str,
    pub update_trigger: &'a str,
}

pub fn sqlite_fts_up_statements(config: &SqliteFtsConfig<'_>) -> Vec<String> {
    let column_list = config.columns.join(", ");
    let virtual_columns = column_list.clone();
    let select_columns = column_list.clone();
    let new_values = config
        .columns
        .iter()
        .map(|column| format!("new.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let update_of_columns = column_list.clone();
    let update_assignments = config
        .columns
        .iter()
        .map(|column| format!("{column} = new.{column}"))
        .collect::<Vec<_>>()
        .join(", ");

    vec![
        format!(
            "CREATE VIRTUAL TABLE IF NOT EXISTS {} USING fts5({}, tokenize='trigram')",
            config.virtual_table, virtual_columns
        ),
        format!("DELETE FROM {}", config.virtual_table),
        format!(
            "INSERT INTO {}(rowid, {}) SELECT id, {} FROM {}",
            config.virtual_table, column_list, select_columns, config.source_table
        ),
        format!(
            "CREATE TRIGGER IF NOT EXISTS {} \
             AFTER INSERT ON {} BEGIN \
               INSERT INTO {}(rowid, {}) VALUES (new.id, {}); \
             END",
            config.insert_trigger,
            config.source_table,
            config.virtual_table,
            column_list,
            new_values,
        ),
        format!(
            "CREATE TRIGGER IF NOT EXISTS {} \
             AFTER DELETE ON {} BEGIN \
               DELETE FROM {} WHERE rowid = old.id; \
             END",
            config.delete_trigger, config.source_table, config.virtual_table
        ),
        format!(
            "CREATE TRIGGER IF NOT EXISTS {} \
             AFTER UPDATE OF {} ON {} BEGIN \
               UPDATE {} SET {} WHERE rowid = new.id; \
             END",
            config.update_trigger,
            update_of_columns,
            config.source_table,
            config.virtual_table,
            update_assignments,
        ),
    ]
}

pub fn sqlite_fts_down_statements(config: &SqliteFtsConfig<'_>) -> Vec<String> {
    vec![
        format!("DROP TRIGGER IF EXISTS {}", config.insert_trigger),
        format!("DROP TRIGGER IF EXISTS {}", config.delete_trigger),
        format!("DROP TRIGGER IF EXISTS {}", config.update_trigger),
        format!("DROP TABLE IF EXISTS {}", config.virtual_table),
    ]
}

pub async fn execute_sqlite_statements(
    manager: &SchemaManager<'_>,
    statements: impl IntoIterator<Item = String>,
    error_context: &str,
) -> Result<(), DbErr> {
    let db = manager.get_connection();

    for sql in statements {
        db.execute_unprepared(&sql)
            .await
            .map_err(|err| DbErr::Custom(format!("{error_context}: {err}")))?;
    }

    Ok(())
}

pub async fn ensure_postgres_extension(
    manager: &SchemaManager<'_>,
    extension_name: &str,
) -> Result<(), DbErr> {
    let sql = Extension::create()
        .name(extension_name)
        .if_not_exists()
        .to_string(PostgresQueryBuilder);

    manager.get_connection().execute_unprepared(&sql).await?;

    Ok(())
}

pub fn postgres_trigram_index(
    index_name: &str,
    table_name: &str,
    column_name: &str,
) -> IndexCreateStatement {
    Index::create()
        .if_not_exists()
        .name(index_name)
        .table(Alias::new(table_name))
        .full_text()
        .col(
            Alias::new(column_name)
                .into_index_column()
                .with_operator_class("gin_trgm_ops"),
        )
        .to_owned()
}

pub fn postgres_drop_index(index_name: &str) -> IndexDropStatement {
    Index::drop().if_exists().name(index_name).to_owned()
}

pub fn mysql_fulltext_index_sql(index_name: &str, table_name: &str, columns: &[&str]) -> String {
    format!(
        "CREATE FULLTEXT INDEX {index_name} ON {table_name} ({}) WITH PARSER ngram",
        columns.join(", ")
    )
}
