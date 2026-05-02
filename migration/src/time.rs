//! 迁移里的时间表达式 helper。

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

const MYSQL_UTC_DATETIME_TYPE: &str = "datetime(6)";

pub fn utc_date_time_column<T>(manager: &SchemaManager<'_>, column: T) -> ColumnDef
where
    T: IntoIden,
{
    utc_date_time_column_for_backend(manager.get_database_backend(), column)
}

pub fn utc_date_time_column_for_backend<T>(backend: DatabaseBackend, column: T) -> ColumnDef
where
    T: IntoIden,
{
    let mut column = ColumnDef::new(column);

    match backend {
        DatabaseBackend::MySql => {
            column.custom(Alias::new(MYSQL_UTC_DATETIME_TYPE));
        }
        _ => {
            column.timestamp_with_time_zone();
        }
    }

    column
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_query::{
        MysqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder,
    };

    #[derive(DeriveIden)]
    enum ExampleTable {
        Table,
        ExpiresAt,
    }

    fn create_table_sql(backend: DatabaseBackend) -> String {
        let table = Table::create()
            .table(ExampleTable::Table)
            .col(utc_date_time_column_for_backend(backend, ExampleTable::ExpiresAt).not_null())
            .to_owned();

        match backend {
            DatabaseBackend::MySql => table.to_string(MysqlQueryBuilder),
            DatabaseBackend::Postgres => table.to_string(PostgresQueryBuilder),
            DatabaseBackend::Sqlite => table.to_string(SqliteQueryBuilder),
            _ => unreachable!("unsupported backend in UTC datetime helper test"),
        }
    }

    #[test]
    fn mysql_uses_datetime_with_microseconds_for_utc_columns() {
        assert!(create_table_sql(DatabaseBackend::MySql).contains("datetime(6) NOT NULL"));
    }

    #[test]
    fn postgres_keeps_timestamp_with_time_zone_for_utc_columns() {
        assert!(
            create_table_sql(DatabaseBackend::Postgres)
                .contains("timestamp with time zone NOT NULL")
        );
    }

    #[test]
    fn sqlite_keeps_timestamp_with_timezone_text_for_utc_columns() {
        assert!(
            create_table_sql(DatabaseBackend::Sqlite)
                .contains("timestamp_with_timezone_text NOT NULL")
        );
    }
}
