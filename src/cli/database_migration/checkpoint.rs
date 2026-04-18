//! CLI 子模块：`checkpoint`。

use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};

use crate::errors::{AsterError, MapAsterErr, Result};
use crate::utils::hash::sha256_hex;

use super::helpers::{
    now_ms, nullable_sql_string, quote_ident, quote_literal, redact_database_url,
};
use super::schema::{ensure_target_empty, total_source_rows};
use super::{CHECKPOINT_TABLE, DatabaseMigrateArgs, MigrationCheckpoint, MigrationMode, TablePlan};

#[derive(Debug)]
pub(super) struct InitializedCheckpoint {
    pub(super) checkpoint: MigrationCheckpoint,
    pub(super) resumed: bool,
}

pub(super) async fn ensure_checkpoint_table(target: &DatabaseConnection) -> Result<()> {
    let backend = target.get_database_backend();
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS {} (\
            {} VARCHAR(64) PRIMARY KEY, \
            {} TEXT NOT NULL, \
            {} TEXT NOT NULL, \
            {} VARCHAR(32) NOT NULL, \
            {} VARCHAR(32) NOT NULL, \
            {} VARCHAR(32) NOT NULL, \
            {} VARCHAR(255) NULL, \
            {} BIGINT NOT NULL DEFAULT 0, \
            {} BIGINT NOT NULL DEFAULT 0, \
            {} BIGINT NOT NULL DEFAULT 0, \
            {} BIGINT NOT NULL DEFAULT 0, \
            {} TEXT NOT NULL, \
            {} TEXT NULL, \
            {} TEXT NULL, \
            {} BIGINT NOT NULL DEFAULT 0, \
            {} BIGINT NOT NULL DEFAULT 0\
        )",
        quote_ident(backend, CHECKPOINT_TABLE),
        quote_ident(backend, "migration_key"),
        quote_ident(backend, "source_database_url"),
        quote_ident(backend, "target_database_url"),
        quote_ident(backend, "mode"),
        quote_ident(backend, "status"),
        quote_ident(backend, "stage"),
        quote_ident(backend, "current_table"),
        quote_ident(backend, "current_table_index"),
        quote_ident(backend, "current_table_offset"),
        quote_ident(backend, "copied_rows"),
        quote_ident(backend, "total_rows"),
        quote_ident(backend, "plan_json"),
        quote_ident(backend, "result_json"),
        quote_ident(backend, "last_error"),
        quote_ident(backend, "heartbeat_at_ms"),
        quote_ident(backend, "updated_at_ms"),
    );

    target
        .execute_raw(Statement::from_string(backend, sql))
        .await
        .map_aster_err(AsterError::database_operation)?;
    Ok(())
}

pub(super) async fn initialize_checkpoint(
    args: &DatabaseMigrateArgs,
    target: &DatabaseConnection,
    plans: &[TablePlan],
) -> Result<InitializedCheckpoint> {
    let migration_key = build_migration_key(
        &args.source_database_url,
        &args.target_database_url,
        MigrationMode::Apply,
    );
    let total_rows = total_source_rows(plans);
    let plan_json = serde_json::to_string(plans).map_err(|error| {
        AsterError::internal_error(format!("failed to serialize migration plan: {error}"))
    })?;

    if let Some(mut checkpoint) = load_checkpoint(target, &migration_key).await? {
        if checkpoint.plan_json != plan_json
            && (checkpoint.copied_rows != 0 || checkpoint.current_table_index != 0)
        {
            return Err(AsterError::validation_error(
                "existing checkpoint does not match the current source plan; target must be reset before retrying",
            ));
        }

        checkpoint.plan_json = plan_json;
        checkpoint.status = "running".to_string();
        checkpoint.last_error = None;
        checkpoint.total_rows = total_rows;
        checkpoint.updated_at_ms = now_ms();
        checkpoint.heartbeat_at_ms = checkpoint.updated_at_ms;
        update_checkpoint(target, &checkpoint).await?;
        return Ok(InitializedCheckpoint {
            checkpoint,
            resumed: true,
        });
    }

    ensure_target_empty(target, plans).await?;
    let now = now_ms();
    let checkpoint = MigrationCheckpoint {
        migration_key,
        source_database_url: redact_database_url(&args.source_database_url),
        target_database_url: redact_database_url(&args.target_database_url),
        mode: MigrationMode::Apply.as_str().to_string(),
        status: "running".to_string(),
        stage: "data_copy".to_string(),
        current_table: None,
        current_table_index: 0,
        current_table_offset: 0,
        copied_rows: 0,
        total_rows,
        plan_json,
        result_json: None,
        last_error: None,
        heartbeat_at_ms: now,
        updated_at_ms: now,
    };
    insert_checkpoint(target, &checkpoint).await?;
    Ok(InitializedCheckpoint {
        checkpoint,
        resumed: false,
    })
}

pub(super) async fn update_checkpoint<C>(db: &C, checkpoint: &MigrationCheckpoint) -> Result<()>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let sql = format!(
        "UPDATE {table_name} SET \
            {source_col} = {source}, \
            {target_col} = {target}, \
            {mode_col} = {mode}, \
            {status_col} = {status}, \
            {stage_col} = {stage}, \
            {current_table_col} = {current_table}, \
            {current_table_index_col} = {current_table_index}, \
            {current_table_offset_col} = {current_table_offset}, \
            {copied_rows_col} = {copied_rows}, \
            {total_rows_col} = {total_rows}, \
            {plan_json_col} = {plan_json}, \
            {result_json_col} = {result_json}, \
            {last_error_col} = {last_error}, \
            {heartbeat_col} = {heartbeat}, \
            {updated_col} = {updated} \
         WHERE {migration_key_col} = {migration_key}",
        table_name = quote_ident(backend, CHECKPOINT_TABLE),
        source_col = quote_ident(backend, "source_database_url"),
        target_col = quote_ident(backend, "target_database_url"),
        mode_col = quote_ident(backend, "mode"),
        status_col = quote_ident(backend, "status"),
        stage_col = quote_ident(backend, "stage"),
        current_table_col = quote_ident(backend, "current_table"),
        current_table_index_col = quote_ident(backend, "current_table_index"),
        current_table_offset_col = quote_ident(backend, "current_table_offset"),
        copied_rows_col = quote_ident(backend, "copied_rows"),
        total_rows_col = quote_ident(backend, "total_rows"),
        plan_json_col = quote_ident(backend, "plan_json"),
        result_json_col = quote_ident(backend, "result_json"),
        last_error_col = quote_ident(backend, "last_error"),
        heartbeat_col = quote_ident(backend, "heartbeat_at_ms"),
        updated_col = quote_ident(backend, "updated_at_ms"),
        migration_key_col = quote_ident(backend, "migration_key"),
        source = quote_literal(&checkpoint.source_database_url),
        target = quote_literal(&checkpoint.target_database_url),
        mode = quote_literal(&checkpoint.mode),
        status = quote_literal(&checkpoint.status),
        stage = quote_literal(&checkpoint.stage),
        current_table = nullable_sql_string(checkpoint.current_table.as_deref()),
        current_table_index = checkpoint.current_table_index,
        current_table_offset = checkpoint.current_table_offset,
        copied_rows = checkpoint.copied_rows,
        total_rows = checkpoint.total_rows,
        plan_json = quote_literal(&checkpoint.plan_json),
        result_json = nullable_sql_string(checkpoint.result_json.as_deref()),
        last_error = nullable_sql_string(checkpoint.last_error.as_deref()),
        heartbeat = checkpoint.heartbeat_at_ms,
        updated = checkpoint.updated_at_ms,
        migration_key = quote_literal(&checkpoint.migration_key),
    );

    db.execute_raw(Statement::from_string(backend, sql))
        .await
        .map_aster_err(AsterError::database_operation)?;
    Ok(())
}

pub(super) async fn mark_checkpoint_failed(
    target: &DatabaseConnection,
    checkpoint: &mut MigrationCheckpoint,
    error: &AsterError,
) -> Result<()> {
    checkpoint.status = "failed".to_string();
    checkpoint.last_error = Some(error.message().to_string());
    checkpoint.updated_at_ms = now_ms();
    checkpoint.heartbeat_at_ms = checkpoint.updated_at_ms;
    update_checkpoint(target, checkpoint).await
}

pub(super) fn resume_message(checkpoint: &MigrationCheckpoint) -> String {
    match checkpoint.current_table.as_deref() {
        Some(table) => format!(
            "resuming checkpoint {} at {} offset {} ({}/{})",
            checkpoint.migration_key,
            table,
            checkpoint.current_table_offset,
            checkpoint.copied_rows,
            checkpoint.total_rows
        ),
        None => format!(
            "resuming checkpoint {} at table index {} ({}/{})",
            checkpoint.migration_key,
            checkpoint.current_table_index,
            checkpoint.copied_rows,
            checkpoint.total_rows
        ),
    }
}

async fn load_checkpoint(
    target: &DatabaseConnection,
    migration_key: &str,
) -> Result<Option<MigrationCheckpoint>> {
    let backend = target.get_database_backend();
    let sql = format!(
        "SELECT \
            {migration_key_col}, {source_col}, {target_col}, {mode_col}, {status_col}, {stage_col}, \
            {current_table_col}, {current_table_index_col}, {current_table_offset_col}, \
            {copied_rows_col}, {total_rows_col}, {plan_json_col}, {result_json_col}, \
            {last_error_col}, {heartbeat_col}, {updated_col} \
         FROM {table_name} \
         WHERE {migration_key_col} = {}",
        quote_literal(migration_key),
        migration_key_col = quote_ident(backend, "migration_key"),
        source_col = quote_ident(backend, "source_database_url"),
        target_col = quote_ident(backend, "target_database_url"),
        mode_col = quote_ident(backend, "mode"),
        status_col = quote_ident(backend, "status"),
        stage_col = quote_ident(backend, "stage"),
        current_table_col = quote_ident(backend, "current_table"),
        current_table_index_col = quote_ident(backend, "current_table_index"),
        current_table_offset_col = quote_ident(backend, "current_table_offset"),
        copied_rows_col = quote_ident(backend, "copied_rows"),
        total_rows_col = quote_ident(backend, "total_rows"),
        plan_json_col = quote_ident(backend, "plan_json"),
        result_json_col = quote_ident(backend, "result_json"),
        last_error_col = quote_ident(backend, "last_error"),
        heartbeat_col = quote_ident(backend, "heartbeat_at_ms"),
        updated_col = quote_ident(backend, "updated_at_ms"),
        table_name = quote_ident(backend, CHECKPOINT_TABLE),
    );

    let Some(row) = target
        .query_one_raw(Statement::from_string(backend, sql))
        .await
        .map_aster_err(AsterError::database_operation)?
    else {
        return Ok(None);
    };

    Ok(Some(MigrationCheckpoint {
        migration_key: row
            .try_get_by_index::<String>(0)
            .map_aster_err(AsterError::database_operation)?,
        source_database_url: redact_database_url(
            &row.try_get_by_index::<String>(1)
                .map_aster_err(AsterError::database_operation)?,
        ),
        target_database_url: redact_database_url(
            &row.try_get_by_index::<String>(2)
                .map_aster_err(AsterError::database_operation)?,
        ),
        mode: row
            .try_get_by_index::<String>(3)
            .map_aster_err(AsterError::database_operation)?,
        status: row
            .try_get_by_index::<String>(4)
            .map_aster_err(AsterError::database_operation)?,
        stage: row
            .try_get_by_index::<String>(5)
            .map_aster_err(AsterError::database_operation)?,
        current_table: row
            .try_get_by_index::<Option<String>>(6)
            .map_aster_err(AsterError::database_operation)?,
        current_table_index: row
            .try_get_by_index::<i64>(7)
            .map_aster_err(AsterError::database_operation)?,
        current_table_offset: row
            .try_get_by_index::<i64>(8)
            .map_aster_err(AsterError::database_operation)?,
        copied_rows: row
            .try_get_by_index::<i64>(9)
            .map_aster_err(AsterError::database_operation)?,
        total_rows: row
            .try_get_by_index::<i64>(10)
            .map_aster_err(AsterError::database_operation)?,
        plan_json: row
            .try_get_by_index::<String>(11)
            .map_aster_err(AsterError::database_operation)?,
        result_json: row
            .try_get_by_index::<Option<String>>(12)
            .map_aster_err(AsterError::database_operation)?,
        last_error: row
            .try_get_by_index::<Option<String>>(13)
            .map_aster_err(AsterError::database_operation)?,
        heartbeat_at_ms: row
            .try_get_by_index::<i64>(14)
            .map_aster_err(AsterError::database_operation)?,
        updated_at_ms: row
            .try_get_by_index::<i64>(15)
            .map_aster_err(AsterError::database_operation)?,
    }))
}

async fn insert_checkpoint<C>(db: &C, checkpoint: &MigrationCheckpoint) -> Result<()>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let sql = format!(
        "INSERT INTO {table_name} (\
            {migration_key_col}, {source_col}, {target_col}, {mode_col}, {status_col}, {stage_col}, \
            {current_table_col}, {current_table_index_col}, {current_table_offset_col}, \
            {copied_rows_col}, {total_rows_col}, {plan_json_col}, {result_json_col}, \
            {last_error_col}, {heartbeat_col}, {updated_col}\
        ) VALUES (\
            {migration_key}, {source}, {target}, {mode}, {status}, {stage}, \
            {current_table}, {current_table_index}, {current_table_offset}, \
            {copied_rows}, {total_rows}, {plan_json}, {result_json}, \
            {last_error}, {heartbeat}, {updated}\
        )",
        table_name = quote_ident(backend, CHECKPOINT_TABLE),
        migration_key_col = quote_ident(backend, "migration_key"),
        source_col = quote_ident(backend, "source_database_url"),
        target_col = quote_ident(backend, "target_database_url"),
        mode_col = quote_ident(backend, "mode"),
        status_col = quote_ident(backend, "status"),
        stage_col = quote_ident(backend, "stage"),
        current_table_col = quote_ident(backend, "current_table"),
        current_table_index_col = quote_ident(backend, "current_table_index"),
        current_table_offset_col = quote_ident(backend, "current_table_offset"),
        copied_rows_col = quote_ident(backend, "copied_rows"),
        total_rows_col = quote_ident(backend, "total_rows"),
        plan_json_col = quote_ident(backend, "plan_json"),
        result_json_col = quote_ident(backend, "result_json"),
        last_error_col = quote_ident(backend, "last_error"),
        heartbeat_col = quote_ident(backend, "heartbeat_at_ms"),
        updated_col = quote_ident(backend, "updated_at_ms"),
        migration_key = quote_literal(&checkpoint.migration_key),
        source = quote_literal(&checkpoint.source_database_url),
        target = quote_literal(&checkpoint.target_database_url),
        mode = quote_literal(&checkpoint.mode),
        status = quote_literal(&checkpoint.status),
        stage = quote_literal(&checkpoint.stage),
        current_table = nullable_sql_string(checkpoint.current_table.as_deref()),
        current_table_index = checkpoint.current_table_index,
        current_table_offset = checkpoint.current_table_offset,
        copied_rows = checkpoint.copied_rows,
        total_rows = checkpoint.total_rows,
        plan_json = quote_literal(&checkpoint.plan_json),
        result_json = nullable_sql_string(checkpoint.result_json.as_deref()),
        last_error = nullable_sql_string(checkpoint.last_error.as_deref()),
        heartbeat = checkpoint.heartbeat_at_ms,
        updated = checkpoint.updated_at_ms,
    );

    db.execute_raw(Statement::from_string(backend, sql))
        .await
        .map_aster_err(AsterError::database_operation)?;
    Ok(())
}

fn build_migration_key(
    source_database_url: &str,
    target_database_url: &str,
    mode: MigrationMode,
) -> String {
    let key = format!(
        "{}\n{}\n{}",
        source_database_url,
        target_database_url,
        mode.as_str()
    );
    sha256_hex(key.as_bytes())
}
