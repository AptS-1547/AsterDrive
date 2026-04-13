use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::{self, IsTerminal};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, FixedOffset};
use clap::Args;
use migration::{Migrator, MigratorTrait};
use sea_orm::sea_query::{Alias, Query};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, TransactionTrait,
    TryGetError, Value,
};
use serde::Serialize;

use crate::db;
use crate::errors::{AsterError, Result};
use crate::utils::hash::sha256_hex;

const COPY_TABLE_ORDER: &[&str] = &[
    "storage_policies",
    "storage_policy_groups",
    "storage_policy_group_items",
    "users",
    "user_profiles",
    "teams",
    "team_members",
    "folders",
    "webdav_accounts",
    "file_blobs",
    "files",
    "file_versions",
    "shares",
    "upload_sessions",
    "upload_session_parts",
    "contact_verification_tokens",
    "system_config",
    "audit_logs",
    "mail_outbox",
    "background_tasks",
    "entity_properties",
    "resource_locks",
    "wopi_sessions",
];

const MIGRATION_TABLE: &str = "seaql_migrations";
const CHECKPOINT_TABLE: &str = "aster_cli_database_migrations";
const DEFAULT_COPY_BATCH_SIZE: i64 = 200;
const PROGRESS_ENV: &str = "ASTER_CLI_PROGRESS";
const COPY_BATCH_SIZE_ENV: &str = "ASTER_CLI_COPY_BATCH_SIZE";
const FAIL_AFTER_BATCHES_ENV: &str = "ASTER_CLI_FAIL_AFTER_BATCHES";

#[derive(Debug, Clone, Args)]
pub struct DatabaseMigrateArgs {
    #[arg(long, env = "ASTER_CLI_SOURCE_DATABASE_URL")]
    pub source_database_url: String,
    #[arg(long, env = "ASTER_CLI_TARGET_DATABASE_URL")]
    pub target_database_url: String,
    #[arg(long, env = "ASTER_CLI_DRY_RUN", default_value_t = false)]
    pub dry_run: bool,
    #[arg(long, env = "ASTER_CLI_VERIFY_ONLY", default_value_t = false)]
    pub verify_only: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum MigrationMode {
    Apply,
    DryRun,
    VerifyOnly,
}

impl MigrationMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Apply => "apply",
            Self::DryRun => "dry_run",
            Self::VerifyOnly => "verify_only",
        }
    }
}

#[derive(Debug, Serialize)]
struct DatabaseEndpointReport {
    database_url: String,
    backend: String,
    pending_migrations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct StageReport {
    name: &'static str,
    status: &'static str,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct TableReport {
    name: String,
    primary_key: Vec<String>,
    source_rows: i64,
    target_rows: i64,
    copied_rows: i64,
    sequence_reset: bool,
}

#[derive(Debug, Clone, Serialize)]
struct CountMismatch {
    table: String,
    source_rows: i64,
    target_rows: i64,
}

#[derive(Debug, Clone, Serialize)]
struct ConstraintCheck {
    table: String,
    constraint: String,
    columns: Vec<String>,
    violations: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
struct VerificationReport {
    checked: bool,
    checked_unique_constraints: usize,
    checked_foreign_keys: usize,
    count_mismatches: Vec<CountMismatch>,
    unique_conflicts: Vec<ConstraintCheck>,
    foreign_key_violations: Vec<ConstraintCheck>,
}

#[derive(Debug, Serialize)]
struct TotalsReport {
    tables: usize,
    source_rows: i64,
    target_rows: i64,
    copied_rows: i64,
    duration_ms: u128,
}

#[derive(Debug, Serialize)]
struct ResumeReport {
    enabled: bool,
    resumed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    migration_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_table: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_table_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_table_offset: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    copied_rows: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_rows: Option<i64>,
}

impl ResumeReport {
    fn disabled() -> Self {
        Self {
            enabled: false,
            resumed: false,
            migration_key: None,
            status: None,
            stage: None,
            current_table: None,
            current_table_index: None,
            current_table_offset: None,
            copied_rows: None,
            total_rows: None,
        }
    }

    fn from_checkpoint(checkpoint: &MigrationCheckpoint, resumed: bool) -> Self {
        Self {
            enabled: true,
            resumed,
            migration_key: Some(checkpoint.migration_key.clone()),
            status: Some(checkpoint.status.clone()),
            stage: Some(checkpoint.stage.clone()),
            current_table: checkpoint.current_table.clone(),
            current_table_index: Some(checkpoint.current_table_index as usize),
            current_table_offset: Some(checkpoint.current_table_offset),
            copied_rows: Some(checkpoint.copied_rows),
            total_rows: Some(checkpoint.total_rows),
        }
    }
}

#[derive(Debug, Serialize)]
struct DatabaseMigrationReport {
    mode: MigrationMode,
    ready_to_cutover: bool,
    rolled_back: bool,
    source: DatabaseEndpointReport,
    target: DatabaseEndpointReport,
    stages: Vec<StageReport>,
    tables: Vec<TableReport>,
    verification: VerificationReport,
    totals: TotalsReport,
    resume: ResumeReport,
}

#[derive(Debug, Clone, Serialize)]
struct ColumnSchema {
    name: String,
    raw_type: String,
    pk_order: i32,
    binding_kind: BindingKind,
}

#[derive(Debug, Clone, Serialize)]
struct TablePlan {
    name: String,
    columns: Vec<ColumnSchema>,
    primary_key: Vec<String>,
    source_rows: i64,
    sequence_reset: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
enum BindingKind {
    Bool,
    Int32,
    Int64,
    Float64,
    String,
    Bytes,
    TimestampWithTimeZone,
}

#[derive(Debug)]
enum CellValue {
    Null,
    Bool(bool),
    Int64(i64),
    Float64(f64),
    String(String),
    Bytes(Vec<u8>),
    Timestamp(DateTime<FixedOffset>),
}

#[derive(Debug)]
struct UniqueIndex {
    table: String,
    name: String,
    columns: Vec<String>,
    is_primary: bool,
}

#[derive(Debug)]
struct ForeignKey {
    table: String,
    name: String,
    column: String,
    referenced_table: String,
    referenced_column: String,
}

#[derive(Debug, Clone)]
struct MigrationCheckpoint {
    migration_key: String,
    source_database_url: String,
    target_database_url: String,
    mode: String,
    status: String,
    stage: String,
    current_table: Option<String>,
    current_table_index: i64,
    current_table_offset: i64,
    copied_rows: i64,
    total_rows: i64,
    plan_json: String,
    result_json: Option<String>,
    last_error: Option<String>,
    heartbeat_at_ms: i64,
    updated_at_ms: i64,
}

#[derive(Debug)]
struct InitializedCheckpoint {
    checkpoint: MigrationCheckpoint,
    resumed: bool,
}

#[derive(Debug)]
struct ApplyExecution {
    target_pending_after: Vec<String>,
    verification: VerificationReport,
    ready_to_cutover: bool,
    stages: Vec<StageReport>,
    checkpoint: MigrationCheckpoint,
    resumed: bool,
}

struct ApplyModeContext<'a> {
    args: &'a DatabaseMigrateArgs,
    source_db: &'a DatabaseConnection,
    target_db: &'a DatabaseConnection,
    source_plans: &'a [TablePlan],
    table_reports: &'a mut [TableReport],
    target_pending_before: &'a [String],
    expected_migrations: &'a [String],
    progress: &'a ProgressReporter,
}

struct ProgressReporter {
    enabled: bool,
}

impl ProgressReporter {
    fn new() -> Self {
        Self {
            enabled: io::stderr().is_terminal() || env_truthy(PROGRESS_ENV),
        }
    }

    fn stage(&self, stage: &str, message: impl AsRef<str>) {
        if self.enabled {
            eprintln!("[database-migrate] {stage}: {}", message.as_ref());
        }
    }

    fn batch(
        &self,
        table_index: usize,
        table_count: usize,
        plan: &TablePlan,
        table_copied: i64,
        overall_copied: i64,
        total_rows: i64,
    ) {
        if !self.enabled {
            return;
        }

        let table_pct = format_percent(table_copied, plan.source_rows);
        let overall_pct = format_percent(overall_copied, total_rows);
        eprintln!(
            "[database-migrate] data_copy: [{}/{}] {} {}/{} rows ({}) total {}/{} ({})",
            table_index + 1,
            table_count,
            plan.name,
            table_copied,
            plan.source_rows,
            table_pct,
            overall_copied,
            total_rows,
            overall_pct
        );
    }
}

pub async fn execute_database_migration(args: &DatabaseMigrateArgs) -> Result<serde_json::Value> {
    if args.dry_run && args.verify_only {
        return Err(AsterError::validation_error(
            "dry-run and verify-only cannot be enabled at the same time",
        ));
    }

    if args.source_database_url == args.target_database_url {
        return Err(AsterError::validation_error(
            "source and target database URLs must not be identical",
        ));
    }

    let mode = if args.dry_run {
        MigrationMode::DryRun
    } else if args.verify_only {
        MigrationMode::VerifyOnly
    } else {
        MigrationMode::Apply
    };

    let started_at = Instant::now();
    let progress = ProgressReporter::new();
    let source_db = connect_database(&args.source_database_url).await?;
    let target_db = connect_database(&args.target_database_url).await?;
    let source_backend = source_db.get_database_backend();
    let target_backend = target_db.get_database_backend();
    validate_backends(source_backend, target_backend)?;

    let expected_migrations = migration_names();
    let source_pending =
        pending_migrations(&source_db, source_backend, &expected_migrations).await?;
    if !source_pending.is_empty() {
        return Err(AsterError::validation_error(format!(
            "source database has pending migrations: {}",
            join_strings(&source_pending)
        )));
    }

    let source_plans = load_source_plans(&source_db).await?;
    let source_rows_total = total_source_rows(&source_plans);
    let target_pending_before =
        pending_migrations(&target_db, target_backend, &expected_migrations).await?;

    progress.stage(
        "preflight",
        format!(
            "source={} target={} pending_source=0 pending_target={}",
            backend_name(source_backend),
            backend_name(target_backend),
            target_pending_before.len()
        ),
    );

    let mut stages = vec![StageReport {
        name: "preflight",
        status: "ok",
        message: format!(
            "source={} target={} pending_source=0 pending_target={}",
            backend_name(source_backend),
            backend_name(target_backend),
            target_pending_before.len()
        ),
    }];
    let mut table_reports = plans_to_reports(&source_plans);
    let mut verification = VerificationReport::default();
    let mut ready_to_cutover = false;
    let mut target_pending_after = target_pending_before.clone();
    let mut resume = ResumeReport::disabled();

    match mode {
        MigrationMode::DryRun => {
            progress.stage(
                "structure_prepare",
                if target_pending_before.is_empty() {
                    "target schema already matches current migrations".to_string()
                } else {
                    format!(
                        "would apply {} pending migrations",
                        target_pending_before.len()
                    )
                },
            );
            stages.push(StageReport {
                name: "structure_prepare",
                status: "planned",
                message: if target_pending_before.is_empty() {
                    "target schema already matches current migrations".to_string()
                } else {
                    format!(
                        "would apply {} pending migrations: {}",
                        target_pending_before.len(),
                        join_strings(&target_pending_before)
                    )
                },
            });
            stages.push(StageReport {
                name: "data_copy",
                status: "planned",
                message: format!(
                    "would copy {} tables and {} rows",
                    source_plans.len(),
                    source_rows_total
                ),
            });
            stages.push(StageReport {
                name: "verification",
                status: "skipped",
                message: "dry-run does not mutate target or execute post-copy verification"
                    .to_string(),
            });
        }
        MigrationMode::VerifyOnly => {
            if !target_pending_before.is_empty() {
                return Err(AsterError::validation_error(format!(
                    "verify-only requires target schema to be current, pending migrations: {}",
                    join_strings(&target_pending_before)
                )));
            }

            progress.stage(
                "structure_prepare",
                "target schema is current; verify-only skips migrations",
            );
            stages.push(StageReport {
                name: "structure_prepare",
                status: "ok",
                message: "target schema is current; verify-only skips migrations".to_string(),
            });
            stages.push(StageReport {
                name: "data_copy",
                status: "skipped",
                message: "verify-only skips data copy".to_string(),
            });

            verification = verify_target(&target_db, &source_plans).await?;
            ready_to_cutover = verification_ready(&verification);
            refresh_target_rows(&target_db, &mut table_reports).await?;
            progress.stage(
                "verification",
                verification_message(&verification, ready_to_cutover),
            );
            stages.push(StageReport {
                name: "verification",
                status: if ready_to_cutover { "ok" } else { "attention" },
                message: verification_message(&verification, ready_to_cutover),
            });
        }
        MigrationMode::Apply => {
            let apply = execute_apply_mode(ApplyModeContext {
                args,
                source_db: &source_db,
                target_db: &target_db,
                source_plans: &source_plans,
                table_reports: &mut table_reports,
                target_pending_before: &target_pending_before,
                expected_migrations: &expected_migrations,
                progress: &progress,
            })
            .await?;
            target_pending_after = apply.target_pending_after;
            verification = apply.verification;
            ready_to_cutover = apply.ready_to_cutover;
            stages.extend(apply.stages);

            let provisional_report = DatabaseMigrationReport {
                mode,
                ready_to_cutover,
                rolled_back: false,
                source: DatabaseEndpointReport {
                    database_url: redact_database_url(&args.source_database_url),
                    backend: backend_name(source_backend).to_string(),
                    pending_migrations: source_pending.clone(),
                },
                target: DatabaseEndpointReport {
                    database_url: redact_database_url(&args.target_database_url),
                    backend: backend_name(target_backend).to_string(),
                    pending_migrations: target_pending_after.clone(),
                },
                stages: stages.clone(),
                tables: table_reports.clone(),
                verification: verification.clone(),
                totals: TotalsReport {
                    tables: source_plans.len(),
                    source_rows: source_rows_total,
                    target_rows: table_reports.iter().map(|report| report.target_rows).sum(),
                    copied_rows: table_reports.iter().map(|report| report.copied_rows).sum(),
                    duration_ms: started_at.elapsed().as_millis(),
                },
                resume: ResumeReport::from_checkpoint(&apply.checkpoint, apply.resumed),
            };

            let result_json = serde_json::to_string(&provisional_report).map_err(|error| {
                AsterError::internal_error(format!(
                    "failed to serialize database migration report: {error}"
                ))
            })?;
            let mut final_checkpoint = apply.checkpoint;
            final_checkpoint.status = if ready_to_cutover {
                "completed".to_string()
            } else {
                "attention".to_string()
            };
            final_checkpoint.stage = if ready_to_cutover {
                "complete".to_string()
            } else {
                "verification".to_string()
            };
            final_checkpoint.result_json = Some(result_json);
            final_checkpoint.last_error = None;
            final_checkpoint.current_table = None;
            final_checkpoint.current_table_index = source_plans.len() as i64;
            final_checkpoint.current_table_offset = 0;
            final_checkpoint.copied_rows =
                table_reports.iter().map(|report| report.copied_rows).sum();
            final_checkpoint.updated_at_ms = now_ms();
            final_checkpoint.heartbeat_at_ms = final_checkpoint.updated_at_ms;
            update_checkpoint(&target_db, &final_checkpoint).await?;
            resume = ResumeReport::from_checkpoint(&final_checkpoint, apply.resumed);
        }
    }

    let report = DatabaseMigrationReport {
        mode,
        ready_to_cutover,
        rolled_back: false,
        source: DatabaseEndpointReport {
            database_url: redact_database_url(&args.source_database_url),
            backend: backend_name(source_backend).to_string(),
            pending_migrations: source_pending,
        },
        target: DatabaseEndpointReport {
            database_url: redact_database_url(&args.target_database_url),
            backend: backend_name(target_backend).to_string(),
            pending_migrations: target_pending_after,
        },
        stages,
        tables: table_reports.clone(),
        verification,
        totals: TotalsReport {
            tables: source_plans.len(),
            source_rows: source_rows_total,
            target_rows: table_reports.iter().map(|report| report.target_rows).sum(),
            copied_rows: table_reports.iter().map(|report| report.copied_rows).sum(),
            duration_ms: started_at.elapsed().as_millis(),
        },
        resume,
    };

    serde_json::to_value(report).map_err(|error| {
        AsterError::internal_error(format!(
            "failed to serialize database migration report: {error}"
        ))
    })
}

async fn execute_apply_mode(ctx: ApplyModeContext<'_>) -> Result<ApplyExecution> {
    ctx.progress
        .stage("structure_prepare", "preparing target schema");
    Migrator::up(ctx.target_db, None)
        .await
        .map_err(|error| AsterError::database_operation(error.to_string()))?;
    let target_backend = ctx.target_db.get_database_backend();
    let target_pending_after =
        pending_migrations(ctx.target_db, target_backend, ctx.expected_migrations).await?;
    if !target_pending_after.is_empty() {
        return Err(AsterError::database_operation(format!(
            "target database still has pending migrations after prepare: {}",
            join_strings(&target_pending_after)
        )));
    }

    ensure_checkpoint_table(ctx.target_db).await?;
    let mut checkpoint = initialize_checkpoint(ctx.args, ctx.target_db, ctx.source_plans).await?;
    let resumed = checkpoint.resumed;
    ctx.progress.stage(
        "resume",
        if resumed {
            resume_message(&checkpoint.checkpoint)
        } else {
            "starting a new migration checkpoint".to_string()
        },
    );

    let target_type_hints =
        match load_target_type_hints(ctx.target_db, target_backend, ctx.source_plans).await {
            Ok(value) => value,
            Err(error) => {
                let _ =
                    mark_checkpoint_failed(ctx.target_db, &mut checkpoint.checkpoint, &error).await;
                return Err(error);
            }
        };

    if let Err(error) = copy_tables_with_resume(
        ctx.source_db,
        ctx.target_db,
        ctx.source_plans,
        &target_type_hints,
        &mut checkpoint.checkpoint,
        ctx.progress,
    )
    .await
    {
        let _ = mark_checkpoint_failed(ctx.target_db, &mut checkpoint.checkpoint, &error).await;
        return Err(error);
    }

    if let Err(error) = reset_sequences(ctx.target_db, ctx.source_plans).await {
        let _ = mark_checkpoint_failed(ctx.target_db, &mut checkpoint.checkpoint, &error).await;
        return Err(error);
    }

    checkpoint.checkpoint.stage = "verification".to_string();
    checkpoint.checkpoint.status = "running".to_string();
    checkpoint.checkpoint.current_table = None;
    checkpoint.checkpoint.current_table_index = ctx.source_plans.len() as i64;
    checkpoint.checkpoint.current_table_offset = 0;
    checkpoint.checkpoint.updated_at_ms = now_ms();
    checkpoint.checkpoint.heartbeat_at_ms = checkpoint.checkpoint.updated_at_ms;
    if let Err(error) = update_checkpoint(ctx.target_db, &checkpoint.checkpoint).await {
        let _ = mark_checkpoint_failed(ctx.target_db, &mut checkpoint.checkpoint, &error).await;
        return Err(error);
    }

    ctx.progress
        .stage("verification", "running post-copy verification");
    let verification = match verify_target(ctx.target_db, ctx.source_plans).await {
        Ok(value) => value,
        Err(error) => {
            let _ = mark_checkpoint_failed(ctx.target_db, &mut checkpoint.checkpoint, &error).await;
            return Err(error);
        }
    };
    let ready_to_cutover = verification_ready(&verification);

    if let Err(error) = refresh_target_rows(ctx.target_db, ctx.table_reports).await {
        let _ = mark_checkpoint_failed(ctx.target_db, &mut checkpoint.checkpoint, &error).await;
        return Err(error);
    }
    for report in &mut *ctx.table_reports {
        report.copied_rows = report.target_rows;
    }

    checkpoint.checkpoint.status = if ready_to_cutover {
        "completed".to_string()
    } else {
        "attention".to_string()
    };
    checkpoint.checkpoint.stage = if ready_to_cutover {
        "complete".to_string()
    } else {
        "verification".to_string()
    };
    checkpoint.checkpoint.current_table = None;
    checkpoint.checkpoint.current_table_index = ctx.source_plans.len() as i64;
    checkpoint.checkpoint.current_table_offset = 0;
    checkpoint.checkpoint.copied_rows = total_source_rows(ctx.source_plans);
    checkpoint.checkpoint.last_error = None;
    checkpoint.checkpoint.updated_at_ms = now_ms();
    checkpoint.checkpoint.heartbeat_at_ms = checkpoint.checkpoint.updated_at_ms;

    let stages = vec![
        StageReport {
            name: "structure_prepare",
            status: "ok",
            message: if ctx.target_pending_before.is_empty() {
                "target schema already matched current migrations".to_string()
            } else {
                format!(
                    "applied {} pending migrations",
                    ctx.target_pending_before.len()
                )
            },
        },
        StageReport {
            name: "data_copy",
            status: "ok",
            message: if resumed {
                format!(
                    "copied {} tables and {} rows (resumed from checkpoint)",
                    ctx.source_plans.len(),
                    total_source_rows(ctx.source_plans)
                )
            } else {
                format!(
                    "copied {} tables and {} rows",
                    ctx.source_plans.len(),
                    total_source_rows(ctx.source_plans)
                )
            },
        },
        StageReport {
            name: "verification",
            status: if ready_to_cutover { "ok" } else { "attention" },
            message: verification_message(&verification, ready_to_cutover),
        },
    ];
    ctx.progress.stage(
        "verification",
        verification_message(&verification, ready_to_cutover),
    );

    Ok(ApplyExecution {
        target_pending_after,
        verification,
        ready_to_cutover,
        stages,
        checkpoint: checkpoint.checkpoint,
        resumed,
    })
}

async fn connect_database(database_url: &str) -> Result<DatabaseConnection> {
    db::connect(&crate::config::DatabaseConfig {
        url: database_url.to_string(),
        pool_size: 1,
        retry_count: 0,
    })
    .await
}

fn validate_backends(source_backend: DbBackend, target_backend: DbBackend) -> Result<()> {
    for (role, backend) in [("source", source_backend), ("target", target_backend)] {
        if !matches!(
            backend,
            DbBackend::Sqlite | DbBackend::Postgres | DbBackend::MySql
        ) {
            return Err(AsterError::validation_error(format!(
                "{role} backend must be sqlite, postgres, or mysql, got {}",
                backend_name(backend)
            )));
        }
    }

    Ok(())
}

fn backend_name(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::MySql => "mysql",
        DbBackend::Postgres => "postgres",
        DbBackend::Sqlite => "sqlite",
        _ => "unknown",
    }
}

fn migration_names() -> Vec<String> {
    Migrator::migrations()
        .into_iter()
        .map(|migration| migration.name().to_string())
        .collect()
}

async fn pending_migrations<C>(
    db: &C,
    backend: DbBackend,
    expected: &[String],
) -> Result<Vec<String>>
where
    C: ConnectionTrait,
{
    let applied = applied_migrations(db, backend).await?;
    let applied_lookup: HashSet<&str> = applied.iter().map(String::as_str).collect();
    let unknown_applied: Vec<String> = applied
        .iter()
        .filter(|name| !expected.iter().any(|expected_name| expected_name == *name))
        .cloned()
        .collect();
    if !unknown_applied.is_empty() {
        return Err(AsterError::validation_error(format!(
            "database contains unknown migration versions: {}",
            join_strings(&unknown_applied)
        )));
    }

    Ok(expected
        .iter()
        .filter(|name| !applied_lookup.contains(name.as_str()))
        .cloned()
        .collect())
}

async fn applied_migrations<C>(db: &C, backend: DbBackend) -> Result<Vec<String>>
where
    C: ConnectionTrait,
{
    if !table_exists(db, backend, MIGRATION_TABLE).await? {
        return Ok(Vec::new());
    }

    let sql = format!(
        "SELECT {} FROM {} ORDER BY {}",
        quote_ident(backend, "version"),
        quote_ident(backend, MIGRATION_TABLE),
        quote_ident(backend, "version")
    );
    let rows = db
        .query_all_raw(Statement::from_string(backend, sql))
        .await
        .map_err(|error| AsterError::database_operation(error.to_string()))?;

    rows.into_iter()
        .map(|row| {
            row.try_get_by_index::<String>(0)
                .map_err(|error| AsterError::database_operation(error.to_string()))
        })
        .collect()
}

async fn table_exists<C>(db: &C, backend: DbBackend, table_name: &str) -> Result<bool>
where
    C: ConnectionTrait,
{
    let sql = match backend {
        DbBackend::Sqlite => format!(
            "SELECT CASE WHEN EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = {}) THEN 1 ELSE 0 END",
            quote_literal(table_name)
        ),
        DbBackend::Postgres => format!(
            "SELECT CASE WHEN EXISTS(SELECT 1 FROM information_schema.tables \
             WHERE table_schema = current_schema() AND table_name = {}) THEN 1 ELSE 0 END",
            quote_literal(table_name)
        ),
        DbBackend::MySql => format!(
            "SELECT CASE WHEN EXISTS(SELECT 1 FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_name = {}) THEN 1 ELSE 0 END",
            quote_literal(table_name)
        ),
        _ => {
            return Err(AsterError::validation_error(
                "unsupported database backend for table existence checks",
            ));
        }
    };

    scalar_i64(db, backend, &sql).await.map(|value| value != 0)
}

async fn load_source_plans(source: &DatabaseConnection) -> Result<Vec<TablePlan>> {
    let backend = source.get_database_backend();
    let existing_tables = source_table_names(source, backend).await?;
    validate_source_tables(&existing_tables)?;

    let existing_lookup: BTreeSet<&str> = existing_tables.iter().map(String::as_str).collect();
    let mut plans = Vec::with_capacity(COPY_TABLE_ORDER.len());
    for table in COPY_TABLE_ORDER {
        if !existing_lookup.contains(*table) {
            return Err(AsterError::validation_error(format!(
                "source database is missing expected table '{}'",
                table
            )));
        }
        plans.push(load_table_plan(source, backend, table).await?);
    }
    Ok(plans)
}

async fn source_table_names<C>(source: &C, backend: DbBackend) -> Result<Vec<String>>
where
    C: ConnectionTrait,
{
    let sql = match backend {
        DbBackend::Sqlite => "SELECT name FROM sqlite_master \
                              WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
            .to_string(),
        DbBackend::Postgres => "SELECT table_name FROM information_schema.tables \
                                WHERE table_schema = current_schema() AND table_type = 'BASE TABLE' \
                                ORDER BY table_name"
            .to_string(),
        DbBackend::MySql => "SELECT table_name FROM information_schema.tables \
                             WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' \
                             ORDER BY table_name"
            .to_string(),
        _ => {
            return Err(AsterError::validation_error(
                "unsupported source backend for table discovery",
            ));
        }
    };

    let rows = source
        .query_all_raw(Statement::from_string(backend, sql))
        .await
        .map_err(|error| AsterError::database_operation(error.to_string()))?;
    rows.into_iter()
        .map(|row| {
            row.try_get_by_index::<String>(0)
                .map_err(|error| AsterError::database_operation(error.to_string()))
        })
        .collect()
}

fn validate_source_tables(existing_tables: &[String]) -> Result<()> {
    let known: BTreeSet<&str> = COPY_TABLE_ORDER
        .iter()
        .copied()
        .chain([MIGRATION_TABLE, CHECKPOINT_TABLE])
        .collect();
    let unexpected: Vec<String> = existing_tables
        .iter()
        .filter(|table| !known.contains(table.as_str()))
        .cloned()
        .collect();

    if !unexpected.is_empty() {
        return Err(AsterError::validation_error(format!(
            "source database contains unsupported tables that would not be migrated: {}",
            join_strings(&unexpected)
        )));
    }

    Ok(())
}

async fn load_table_plan<C>(db: &C, backend: DbBackend, table_name: &str) -> Result<TablePlan>
where
    C: ConnectionTrait,
{
    let columns = match backend {
        DbBackend::Sqlite => load_sqlite_columns(db, table_name).await?,
        DbBackend::Postgres => load_postgres_columns(db, table_name).await?,
        DbBackend::MySql => load_mysql_columns(db, table_name).await?,
        _ => {
            return Err(AsterError::validation_error(
                "unsupported source backend for schema inspection",
            ));
        }
    };

    if columns.is_empty() {
        return Err(AsterError::validation_error(format!(
            "source table '{}' has no columns",
            table_name
        )));
    }

    let mut primary_key_pairs: Vec<(i32, String)> = columns
        .iter()
        .filter(|column| column.pk_order > 0)
        .map(|column| (column.pk_order, column.name.clone()))
        .collect();
    primary_key_pairs.sort_by_key(|(pk_order, _)| *pk_order);
    let primary_key = primary_key_pairs
        .into_iter()
        .map(|(_, name)| name)
        .collect::<Vec<_>>();
    let source_rows = count_rows(db, backend, table_name).await?;
    let sequence_reset = columns.iter().any(|column| {
        column.name == "id" && column.pk_order == 1 && binding_kind_is_integer(column.binding_kind)
    });

    Ok(TablePlan {
        name: table_name.to_string(),
        columns,
        primary_key,
        source_rows,
        sequence_reset,
    })
}

async fn load_sqlite_columns<C>(db: &C, table_name: &str) -> Result<Vec<ColumnSchema>>
where
    C: ConnectionTrait,
{
    let sql = format!("PRAGMA table_info({})", quote_sqlite_literal(table_name));
    let rows = db
        .query_all_raw(Statement::from_string(DbBackend::Sqlite, sql))
        .await
        .map_err(|error| AsterError::database_operation(error.to_string()))?;

    rows.into_iter()
        .map(|row| {
            let name: String = row
                .try_get("", "name")
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            let raw_type: String = row
                .try_get("", "type")
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            let pk_order: i32 = row
                .try_get("", "pk")
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            Ok(ColumnSchema {
                name,
                binding_kind: binding_kind_from_raw_type(DbBackend::Sqlite, &raw_type),
                raw_type,
                pk_order,
            })
        })
        .collect()
}

async fn load_postgres_columns<C>(db: &C, table_name: &str) -> Result<Vec<ColumnSchema>>
where
    C: ConnectionTrait,
{
    let sql = format!(
        "SELECT column_name, udt_name \
         FROM information_schema.columns \
         WHERE table_schema = current_schema() AND table_name = {} \
         ORDER BY ordinal_position",
        quote_literal(table_name)
    );
    let pk_lookup = load_primary_key_lookup(
        db,
        DbBackend::Postgres,
        &format!(
            "SELECT kcu.column_name, kcu.ordinal_position \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage kcu \
               ON tc.constraint_name = kcu.constraint_name \
              AND tc.table_schema = kcu.table_schema \
             WHERE tc.constraint_type = 'PRIMARY KEY' \
               AND tc.table_schema = current_schema() \
               AND tc.table_name = {} \
             ORDER BY kcu.ordinal_position",
            quote_literal(table_name)
        ),
    )
    .await?;

    let rows = db
        .query_all_raw(Statement::from_string(DbBackend::Postgres, sql))
        .await
        .map_err(|error| AsterError::database_operation(error.to_string()))?;

    rows.into_iter()
        .map(|row| {
            let name = row
                .try_get_by_index::<String>(0)
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            let raw_type = row
                .try_get_by_index::<String>(1)
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            Ok(ColumnSchema {
                pk_order: *pk_lookup.get(&name).unwrap_or(&0),
                binding_kind: binding_kind_from_raw_type(DbBackend::Postgres, &raw_type),
                name,
                raw_type,
            })
        })
        .collect()
}

async fn load_mysql_columns<C>(db: &C, table_name: &str) -> Result<Vec<ColumnSchema>>
where
    C: ConnectionTrait,
{
    let sql = format!(
        "SELECT column_name, column_type \
         FROM information_schema.columns \
         WHERE table_schema = DATABASE() AND table_name = {} \
         ORDER BY ordinal_position",
        quote_literal(table_name)
    );
    let pk_lookup = load_primary_key_lookup(
        db,
        DbBackend::MySql,
        &format!(
            "SELECT column_name, ordinal_position \
             FROM information_schema.key_column_usage \
             WHERE table_schema = DATABASE() \
               AND table_name = {} \
               AND constraint_name = 'PRIMARY' \
             ORDER BY ordinal_position",
            quote_literal(table_name)
        ),
    )
    .await?;

    let rows = db
        .query_all_raw(Statement::from_string(DbBackend::MySql, sql))
        .await
        .map_err(|error| AsterError::database_operation(error.to_string()))?;

    rows.into_iter()
        .map(|row| {
            let name = row
                .try_get_by_index::<String>(0)
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            let raw_type = row
                .try_get_by_index::<String>(1)
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            Ok(ColumnSchema {
                pk_order: *pk_lookup.get(&name).unwrap_or(&0),
                binding_kind: binding_kind_from_raw_type(DbBackend::MySql, &raw_type),
                name,
                raw_type,
            })
        })
        .collect()
}

async fn load_primary_key_lookup<C>(
    db: &C,
    backend: DbBackend,
    sql: &str,
) -> Result<BTreeMap<String, i32>>
where
    C: ConnectionTrait,
{
    let rows = db
        .query_all_raw(Statement::from_string(backend, sql))
        .await
        .map_err(|error| AsterError::database_operation(error.to_string()))?;
    let mut lookup = BTreeMap::new();
    for row in rows {
        let column_name = row
            .try_get_by_index::<String>(0)
            .map_err(|error| AsterError::database_operation(error.to_string()))?;
        let ordinal = if let Ok(value) = row.try_get_by_index::<i32>(1) {
            value
        } else if let Ok(value) = row.try_get_by_index::<u32>(1) {
            i32::try_from(value).map_err(|_| {
                AsterError::database_operation(format!(
                    "primary key ordinal position {value} does not fit into i32"
                ))
            })?
        } else {
            return Err(AsterError::database_operation(
                "failed to decode primary key ordinal position".to_string(),
            ));
        };
        lookup.insert(column_name, ordinal);
    }
    Ok(lookup)
}

fn plans_to_reports(plans: &[TablePlan]) -> Vec<TableReport> {
    plans
        .iter()
        .map(|plan| TableReport {
            name: plan.name.clone(),
            primary_key: plan.primary_key.clone(),
            source_rows: plan.source_rows,
            target_rows: 0,
            copied_rows: 0,
            sequence_reset: plan.sequence_reset,
        })
        .collect()
}

async fn ensure_target_empty<C>(target: &C, plans: &[TablePlan]) -> Result<()>
where
    C: ConnectionTrait,
{
    let backend = target.get_database_backend();
    let mut non_empty = Vec::new();
    for plan in plans {
        let count = count_rows(target, backend, &plan.name).await?;
        if count != 0 {
            non_empty.push(format!("{}={count}", plan.name));
        }
    }

    if non_empty.is_empty() {
        return Ok(());
    }

    Err(AsterError::validation_error(format!(
        "target database must be empty before migration; found rows in {}",
        non_empty.join(", ")
    )))
}

async fn copy_tables_with_resume(
    source: &DatabaseConnection,
    target: &DatabaseConnection,
    plans: &[TablePlan],
    target_type_hints: &BTreeMap<String, BTreeMap<String, BindingKind>>,
    checkpoint: &mut MigrationCheckpoint,
    progress: &ProgressReporter,
) -> Result<()> {
    let batch_size = copy_batch_size()?;
    let fail_after_batches = fail_after_batches()?;
    let source_backend = source.get_database_backend();
    let start_table_index = checkpoint.current_table_index.max(0) as usize;
    let mut committed_batches = 0_i64;

    for table_index in start_table_index..plans.len() {
        let plan = &plans[table_index];
        let type_hints = target_type_hints.get(&plan.name).ok_or_else(|| {
            AsterError::validation_error(format!(
                "missing target column type hints for table '{}'",
                plan.name
            ))
        })?;
        let mut offset = if table_index == start_table_index {
            checkpoint.current_table_offset
        } else {
            0
        };

        checkpoint.stage = "data_copy".to_string();
        checkpoint.status = "running".to_string();
        checkpoint.current_table = Some(plan.name.clone());
        checkpoint.current_table_index = table_index as i64;
        checkpoint.updated_at_ms = now_ms();
        checkpoint.heartbeat_at_ms = checkpoint.updated_at_ms;
        update_checkpoint(target, checkpoint).await?;

        while offset < plan.source_rows {
            let rows =
                fetch_source_batch(source, source_backend, plan, type_hints, offset, batch_size)
                    .await?;
            if rows.is_empty() {
                break;
            }

            let txn = target
                .begin()
                .await
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            insert_batch(&txn, plan, &rows).await.map_err(|error| {
                AsterError::database_operation(format!(
                    "failed to write target batch for '{}': {error}",
                    plan.name
                ))
            })?;

            offset += rows.len() as i64;
            checkpoint.current_table = Some(plan.name.clone());
            checkpoint.current_table_index = table_index as i64;
            checkpoint.current_table_offset = offset;
            checkpoint.copied_rows += rows.len() as i64;
            checkpoint.updated_at_ms = now_ms();
            checkpoint.heartbeat_at_ms = checkpoint.updated_at_ms;
            update_checkpoint(&txn, checkpoint).await?;
            txn.commit()
                .await
                .map_err(|error| AsterError::database_operation(error.to_string()))?;

            progress.batch(
                table_index,
                plans.len(),
                plan,
                offset,
                checkpoint.copied_rows,
                checkpoint.total_rows,
            );

            committed_batches += 1;
            if let Some(limit) = fail_after_batches
                && committed_batches >= limit
            {
                return Err(AsterError::internal_error(
                    "forced failure after committed batch for resume-path verification",
                ));
            }
        }

        checkpoint.current_table = None;
        checkpoint.current_table_index = (table_index + 1) as i64;
        checkpoint.current_table_offset = 0;
        checkpoint.updated_at_ms = now_ms();
        checkpoint.heartbeat_at_ms = checkpoint.updated_at_ms;
        update_checkpoint(target, checkpoint).await?;
    }

    Ok(())
}

async fn fetch_source_batch(
    source: &DatabaseConnection,
    source_backend: DbBackend,
    plan: &TablePlan,
    target_type_hints: &BTreeMap<String, BindingKind>,
    offset: i64,
    limit: i64,
) -> Result<Vec<Vec<Value>>> {
    let select_columns = plan
        .columns
        .iter()
        .map(|column| quote_ident(source_backend, &column.name))
        .collect::<Vec<_>>()
        .join(", ");
    let order_by = if plan.primary_key.is_empty() {
        String::new()
    } else {
        format!(
            " ORDER BY {}",
            plan.primary_key
                .iter()
                .map(|column| quote_ident(source_backend, column))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let sql = format!(
        "SELECT {select_columns} FROM {}{order_by} LIMIT {limit} OFFSET {offset}",
        quote_ident(source_backend, &plan.name),
    );
    let rows = source
        .query_all_raw(Statement::from_string(source_backend, sql))
        .await
        .map_err(|error| AsterError::database_operation(error.to_string()))?;

    rows.into_iter()
        .map(|row| decode_row_values(&row, plan, target_type_hints))
        .collect()
}

fn decode_row_values(
    row: &QueryResult,
    plan: &TablePlan,
    target_type_hints: &BTreeMap<String, BindingKind>,
) -> Result<Vec<Value>> {
    plan.columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let target_kind = target_type_hints
                .get(&column.name)
                .copied()
                .ok_or_else(|| {
                    AsterError::validation_error(format!(
                        "missing target type hint for {}.{}",
                        plan.name, column.name
                    ))
                })?;
            let cell = decode_source_cell(
                row,
                index,
                column.binding_kind,
                &column.raw_type,
                &plan.name,
                &column.name,
            )?;
            cell_into_target_value(cell, target_kind, &plan.name, &column.name)
        })
        .collect()
}

fn decode_source_cell(
    row: &QueryResult,
    index: usize,
    source_kind: BindingKind,
    raw_type: &str,
    table_name: &str,
    column_name: &str,
) -> Result<CellValue> {
    let decode_error = |error: TryGetError| {
        AsterError::database_operation(format!(
            "failed to decode {}.{} as '{}': {error:?}",
            table_name, column_name, raw_type
        ))
    };

    match source_kind {
        BindingKind::Bool => {
            if let Ok(value) = row.try_get_by_index_nullable::<Option<bool>>(index) {
                return Ok(value.map_or(CellValue::Null, CellValue::Bool));
            }
            let value = row
                .try_get_by_index_nullable::<Option<i32>>(index)
                .map_err(decode_error)?;
            Ok(value.map_or(CellValue::Null, |value| CellValue::Bool(value != 0)))
        }
        BindingKind::Int32 | BindingKind::Int64 => {
            if let Ok(value) = row.try_get_by_index_nullable::<Option<i64>>(index) {
                return Ok(value.map_or(CellValue::Null, CellValue::Int64));
            }
            if let Ok(value) = row.try_get_by_index_nullable::<Option<i32>>(index) {
                return Ok(
                    value.map_or(CellValue::Null, |value| CellValue::Int64(i64::from(value)))
                );
            }
            if let Ok(value) = row.try_get_by_index_nullable::<Option<u64>>(index) {
                return match value {
                    Some(value) => Ok(CellValue::Int64(i64::try_from(value).map_err(|_| {
                        AsterError::database_operation(format!(
                            "failed to decode {}.{} as '{}': u64 value {value} does not fit into i64",
                            table_name, column_name, raw_type
                        ))
                    })?)),
                    None => Ok(CellValue::Null),
                };
            }
            if let Ok(value) = row.try_get_by_index_nullable::<Option<u32>>(index) {
                return Ok(
                    value.map_or(CellValue::Null, |value| CellValue::Int64(i64::from(value)))
                );
            }
            let value = row
                .try_get_by_index_nullable::<Option<bool>>(index)
                .map_err(decode_error)?;
            Ok(value.map_or(CellValue::Null, |value| {
                CellValue::Int64(if value { 1 } else { 0 })
            }))
        }
        BindingKind::Float64 => {
            if let Ok(value) = row.try_get_by_index_nullable::<Option<f64>>(index) {
                return Ok(value.map_or(CellValue::Null, CellValue::Float64));
            }
            if let Ok(value) = row.try_get_by_index_nullable::<Option<f32>>(index) {
                return Ok(value.map_or(CellValue::Null, |value| CellValue::Float64(value as f64)));
            }
            let value = row
                .try_get_by_index_nullable::<Option<i64>>(index)
                .map_err(decode_error)?;
            Ok(value.map_or(CellValue::Null, |value| CellValue::Float64(value as f64)))
        }
        BindingKind::Bytes => {
            let value = row
                .try_get_by_index_nullable::<Option<Vec<u8>>>(index)
                .map_err(decode_error)?;
            Ok(value.map_or(CellValue::Null, CellValue::Bytes))
        }
        BindingKind::TimestampWithTimeZone => {
            let value = row
                .try_get_by_index_nullable::<Option<DateTime<FixedOffset>>>(index)
                .map_err(decode_error)?;
            Ok(value.map_or(CellValue::Null, CellValue::Timestamp))
        }
        BindingKind::String => {
            let value = row
                .try_get_by_index_nullable::<Option<String>>(index)
                .map_err(decode_error)?;
            Ok(value.map_or(CellValue::Null, CellValue::String))
        }
    }
}

fn cell_into_target_value(
    cell: CellValue,
    target_kind: BindingKind,
    table_name: &str,
    column_name: &str,
) -> Result<Value> {
    let conversion_error = |detail: &str| {
        AsterError::database_operation(format!(
            "failed to convert {}.{} for target binding: {detail}",
            table_name, column_name
        ))
    };

    Ok(match target_kind {
        BindingKind::Bool => match cell {
            CellValue::Null => Option::<bool>::None.into(),
            CellValue::Bool(value) => Some(value).into(),
            CellValue::Int64(value) => Some(value != 0).into(),
            CellValue::String(value) => Some(
                parse_bool(&value)
                    .ok_or_else(|| conversion_error("string value is not a valid boolean"))?,
            )
            .into(),
            _ => {
                return Err(conversion_error(
                    "unsupported source type for boolean target",
                ));
            }
        },
        BindingKind::Int32 => match cell {
            CellValue::Null => Option::<i32>::None.into(),
            CellValue::Bool(value) => Some(if value { 1 } else { 0 }).into(),
            CellValue::Int64(value) => Some(
                i32::try_from(value)
                    .map_err(|_| conversion_error("integer overflow while converting to i32"))?,
            )
            .into(),
            CellValue::String(value) => Some(
                value
                    .parse::<i32>()
                    .map_err(|_| conversion_error("string value is not a valid i32"))?,
            )
            .into(),
            _ => return Err(conversion_error("unsupported source type for int32 target")),
        },
        BindingKind::Int64 => match cell {
            CellValue::Null => Option::<i64>::None.into(),
            CellValue::Bool(value) => Some(if value { 1_i64 } else { 0_i64 }).into(),
            CellValue::Int64(value) => Some(value).into(),
            CellValue::String(value) => Some(
                value
                    .parse::<i64>()
                    .map_err(|_| conversion_error("string value is not a valid i64"))?,
            )
            .into(),
            _ => return Err(conversion_error("unsupported source type for int64 target")),
        },
        BindingKind::Float64 => match cell {
            CellValue::Null => Option::<f64>::None.into(),
            CellValue::Bool(value) => Some(if value { 1_f64 } else { 0_f64 }).into(),
            CellValue::Int64(value) => Some(value as f64).into(),
            CellValue::Float64(value) => Some(value).into(),
            CellValue::String(value) => Some(
                value
                    .parse::<f64>()
                    .map_err(|_| conversion_error("string value is not a valid f64"))?,
            )
            .into(),
            _ => return Err(conversion_error("unsupported source type for float target")),
        },
        BindingKind::String => match cell {
            CellValue::Null => Option::<String>::None.into(),
            CellValue::Bool(value) => Some(value.to_string()).into(),
            CellValue::Int64(value) => Some(value.to_string()).into(),
            CellValue::Float64(value) => Some(value.to_string()).into(),
            CellValue::String(value) => Some(value).into(),
            CellValue::Timestamp(value) => Some(value.to_rfc3339()).into(),
            CellValue::Bytes(_) => {
                return Err(conversion_error(
                    "cannot losslessly convert bytes into string",
                ));
            }
        },
        BindingKind::Bytes => match cell {
            CellValue::Null => Option::<Vec<u8>>::None.into(),
            CellValue::Bytes(value) => Some(value).into(),
            _ => return Err(conversion_error("unsupported source type for bytes target")),
        },
        BindingKind::TimestampWithTimeZone => match cell {
            CellValue::Null => Option::<DateTime<FixedOffset>>::None.into(),
            CellValue::Timestamp(value) => Some(value).into(),
            CellValue::String(value) => Some(parse_timestamp(&value).ok_or_else(|| {
                conversion_error("string value is not a valid RFC3339 timestamp")
            })?)
            .into(),
            _ => {
                return Err(conversion_error(
                    "unsupported source type for timestamp target",
                ));
            }
        },
    })
}

async fn insert_batch<C>(target: &C, plan: &TablePlan, rows: &[Vec<Value>]) -> Result<()>
where
    C: ConnectionTrait,
{
    if rows.is_empty() {
        return Ok(());
    }

    let mut insert = Query::insert();
    insert.into_table(Alias::new(plan.name.as_str()));
    insert.columns(
        plan.columns
            .iter()
            .map(|column| Alias::new(column.name.as_str())),
    );
    for values in rows {
        insert
            .values(values.iter().cloned().map(Into::into))
            .map_err(|error| AsterError::database_operation(error.to_string()))?;
    }

    target.execute(&insert).await.map_err(|error| {
        AsterError::database_operation(format!(
            "failed to insert batch into '{}': {error}",
            plan.name
        ))
    })?;
    Ok(())
}

async fn reset_sequences(target: &DatabaseConnection, plans: &[TablePlan]) -> Result<()> {
    let backend = target.get_database_backend();
    for plan in plans.iter().filter(|plan| plan.sequence_reset) {
        match backend {
            DbBackend::Postgres => {
                let table_ident = quote_ident(DbBackend::Postgres, &plan.name);
                let sql = format!(
                    "SELECT setval( \
                        pg_get_serial_sequence({}, {}), \
                        COALESCE((SELECT MAX(id) FROM {table_ident}), 1), \
                        EXISTS (SELECT 1 FROM {table_ident}) \
                    )",
                    quote_literal(&plan.name),
                    quote_literal("id"),
                );
                target
                    .execute_raw(Statement::from_string(DbBackend::Postgres, sql))
                    .await
                    .map_err(|error| AsterError::database_operation(error.to_string()))?;
            }
            DbBackend::MySql => {
                let next_id = scalar_i64(
                    target,
                    DbBackend::MySql,
                    &format!(
                        "SELECT COALESCE(MAX(id), 0) + 1 FROM {}",
                        quote_ident(DbBackend::MySql, &plan.name)
                    ),
                )
                .await?;
                let sql = format!(
                    "ALTER TABLE {} AUTO_INCREMENT = {}",
                    quote_ident(DbBackend::MySql, &plan.name),
                    next_id.max(1)
                );
                target
                    .execute_raw(Statement::from_string(DbBackend::MySql, sql))
                    .await
                    .map_err(|error| AsterError::database_operation(error.to_string()))?;
            }
            DbBackend::Sqlite => {
                // SQLite 在显式插入主键后会自动维护 rowid/autoincrement 状态，
                // 这里不额外改 sqlite_sequence，避免和系统表实现细节耦合。
            }
            _ => {
                return Err(AsterError::validation_error(
                    "unsupported database backend for sequence reset",
                ));
            }
        }
    }
    Ok(())
}

async fn refresh_target_rows(
    target: &DatabaseConnection,
    reports: &mut [TableReport],
) -> Result<()> {
    let backend = target.get_database_backend();
    for report in reports {
        report.target_rows = count_rows(target, backend, &report.name).await?;
    }
    Ok(())
}

async fn load_target_type_hints(
    target: &DatabaseConnection,
    backend: DbBackend,
    plans: &[TablePlan],
) -> Result<BTreeMap<String, BTreeMap<String, BindingKind>>> {
    let mut table_hints = BTreeMap::new();
    for plan in plans {
        let rows = load_column_type_rows(target, backend, &plan.name).await?;
        let mut hints = BTreeMap::new();
        for (column_name, raw_type) in rows {
            hints.insert(column_name, binding_kind_from_raw_type(backend, &raw_type));
        }

        for column in &plan.columns {
            if !hints.contains_key(&column.name) {
                return Err(AsterError::validation_error(format!(
                    "target table '{}' is missing column '{}'",
                    plan.name, column.name
                )));
            }
        }

        table_hints.insert(plan.name.clone(), hints);
    }

    Ok(table_hints)
}

async fn load_column_type_rows<C>(
    db: &C,
    backend: DbBackend,
    table_name: &str,
) -> Result<Vec<(String, String)>>
where
    C: ConnectionTrait,
{
    let sql = match backend {
        DbBackend::Sqlite => format!("PRAGMA table_info({})", quote_sqlite_literal(table_name)),
        DbBackend::Postgres => format!(
            "SELECT column_name, udt_name \
             FROM information_schema.columns \
             WHERE table_schema = current_schema() AND table_name = {} \
             ORDER BY ordinal_position",
            quote_literal(table_name)
        ),
        DbBackend::MySql => format!(
            "SELECT column_name, column_type \
             FROM information_schema.columns \
             WHERE table_schema = DATABASE() AND table_name = {} \
             ORDER BY ordinal_position",
            quote_literal(table_name)
        ),
        _ => {
            return Err(AsterError::validation_error(
                "unsupported database backend for type hints",
            ));
        }
    };

    let rows = db
        .query_all_raw(Statement::from_string(backend, sql))
        .await
        .map_err(|error| AsterError::database_operation(error.to_string()))?;

    match backend {
        DbBackend::Sqlite => rows
            .into_iter()
            .map(|row| {
                let name: String = row
                    .try_get("", "name")
                    .map_err(|error| AsterError::database_operation(error.to_string()))?;
                let raw_type: String = row
                    .try_get("", "type")
                    .map_err(|error| AsterError::database_operation(error.to_string()))?;
                Ok((name, raw_type))
            })
            .collect(),
        _ => rows
            .into_iter()
            .map(|row| {
                let name = row
                    .try_get_by_index::<String>(0)
                    .map_err(|error| AsterError::database_operation(error.to_string()))?;
                let raw_type = row
                    .try_get_by_index::<String>(1)
                    .map_err(|error| AsterError::database_operation(error.to_string()))?;
                Ok((name, raw_type))
            })
            .collect(),
    }
}

fn binding_kind_from_raw_type(backend: DbBackend, raw_type: &str) -> BindingKind {
    let normalized = raw_type.to_ascii_lowercase();
    match backend {
        DbBackend::Postgres => {
            if normalized == "bool" {
                BindingKind::Bool
            } else if normalized == "int8" {
                BindingKind::Int64
            } else if matches!(normalized.as_str(), "int2" | "int4") {
                BindingKind::Int32
            } else if normalized == "float4" || normalized == "float8" || normalized == "numeric" {
                BindingKind::Float64
            } else if normalized == "bytea" {
                BindingKind::Bytes
            } else if normalized.contains("timestamp") || normalized == "timestamptz" {
                BindingKind::TimestampWithTimeZone
            } else {
                BindingKind::String
            }
        }
        DbBackend::MySql => {
            if normalized.starts_with("tinyint(1)") || normalized == "boolean" {
                BindingKind::Bool
            } else if normalized.starts_with("bigint") {
                BindingKind::Int64
            } else if normalized.contains("int") {
                BindingKind::Int32
            } else if normalized.contains("double")
                || normalized.contains("float")
                || normalized.contains("decimal")
            {
                BindingKind::Float64
            } else if normalized.contains("blob") || normalized.contains("binary") {
                BindingKind::Bytes
            } else if normalized.contains("timestamp") || normalized.contains("datetime") {
                BindingKind::TimestampWithTimeZone
            } else {
                BindingKind::String
            }
        }
        DbBackend::Sqlite => {
            if normalized.contains("bool") {
                BindingKind::Bool
            } else if normalized.contains("timestamp") || normalized.contains("datetime") {
                BindingKind::TimestampWithTimeZone
            } else if normalized.contains("blob") {
                BindingKind::Bytes
            } else if normalized.contains("double")
                || normalized.contains("float")
                || normalized.contains("real")
                || normalized.contains("decimal")
            {
                BindingKind::Float64
            } else if normalized.contains("int") {
                BindingKind::Int64
            } else {
                BindingKind::String
            }
        }
        _ => BindingKind::String,
    }
}

async fn verify_target<C>(target: &C, plans: &[TablePlan]) -> Result<VerificationReport>
where
    C: ConnectionTrait,
{
    let mut verification = VerificationReport {
        checked: true,
        ..Default::default()
    };
    let target_backend = target.get_database_backend();

    for plan in plans {
        let target_rows = count_rows(target, target_backend, &plan.name).await?;
        if target_rows != plan.source_rows {
            verification.count_mismatches.push(CountMismatch {
                table: plan.name.clone(),
                source_rows: plan.source_rows,
                target_rows,
            });
        }
    }

    let unique_indexes = load_unique_indexes(target, target_backend, plans).await?;
    verification.checked_unique_constraints = unique_indexes.len() + 2;
    for index in unique_indexes {
        let violations =
            count_duplicate_groups(target, target_backend, &index.table, &index.columns).await?;
        if violations != 0 {
            verification.unique_conflicts.push(ConstraintCheck {
                table: index.table,
                constraint: if index.is_primary {
                    format!("{} (primary key)", index.name)
                } else {
                    index.name
                },
                columns: index.columns,
                violations,
            });
        }
    }

    for (table, name, columns) in live_name_unique_checks() {
        let violations =
            count_expression_duplicates(target, target_backend, table, columns).await?;
        if violations != 0 {
            verification.unique_conflicts.push(ConstraintCheck {
                table: table.to_string(),
                constraint: name.to_string(),
                columns: columns.iter().map(|value| value.to_string()).collect(),
                violations,
            });
        }
    }

    let foreign_keys = load_foreign_keys(target, target_backend, plans).await?;
    verification.checked_foreign_keys = foreign_keys.len();
    for foreign_key in foreign_keys {
        let violations = count_foreign_key_violations(target, target_backend, &foreign_key).await?;
        if violations != 0 {
            verification.foreign_key_violations.push(ConstraintCheck {
                table: foreign_key.table,
                constraint: foreign_key.name,
                columns: vec![foreign_key.column],
                violations,
            });
        }
    }

    Ok(verification)
}

fn verification_ready(verification: &VerificationReport) -> bool {
    verification.count_mismatches.is_empty()
        && verification.unique_conflicts.is_empty()
        && verification.foreign_key_violations.is_empty()
}

fn verification_message(verification: &VerificationReport, ready: bool) -> String {
    if ready {
        return format!(
            "row counts matched; checked {} unique constraints and {} foreign keys",
            verification.checked_unique_constraints, verification.checked_foreign_keys
        );
    }

    format!(
        "counts={} unique_conflicts={} foreign_key_violations={}",
        verification.count_mismatches.len(),
        verification.unique_conflicts.len(),
        verification.foreign_key_violations.len()
    )
}

async fn load_unique_indexes<C>(
    db: &C,
    backend: DbBackend,
    plans: &[TablePlan],
) -> Result<Vec<UniqueIndex>>
where
    C: ConnectionTrait,
{
    let table_filter = COPY_TABLE_ORDER
        .iter()
        .map(|table| quote_literal(table))
        .collect::<Vec<_>>()
        .join(", ");
    match backend {
        DbBackend::Postgres => {
            let sql = format!(
                "SELECT t.relname AS table_name, i.relname AS index_name, \
                        string_agg(a.attname, ',' ORDER BY cols.ordinality) AS columns, \
                        idx.indisprimary AS is_primary \
                 FROM pg_index idx \
                 JOIN pg_class t ON t.oid = idx.indrelid \
                 JOIN pg_namespace ns ON ns.oid = t.relnamespace \
                 JOIN pg_class i ON i.oid = idx.indexrelid \
                 JOIN LATERAL unnest(idx.indkey) WITH ORDINALITY AS cols(attnum, ordinality) ON TRUE \
                 JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = cols.attnum \
                 WHERE ns.nspname = current_schema() \
                   AND t.relname IN ({table_filter}) \
                   AND idx.indisunique \
                   AND idx.indexprs IS NULL \
                 GROUP BY t.relname, i.relname, idx.indisprimary \
                 ORDER BY t.relname, i.relname"
            );
            let rows = db
                .query_all_raw(Statement::from_string(backend, sql))
                .await
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            rows.into_iter()
                .map(|row| {
                    Ok(UniqueIndex {
                        table: row
                            .try_get_by_index::<String>(0)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                        name: row
                            .try_get_by_index::<String>(1)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                        columns: row
                            .try_get_by_index::<String>(2)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?
                            .split(',')
                            .map(str::to_string)
                            .collect(),
                        is_primary: row
                            .try_get_by_index::<bool>(3)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                    })
                })
                .collect()
        }
        DbBackend::MySql => {
            let sql = format!(
                "SELECT table_name, index_name, \
                        GROUP_CONCAT(column_name ORDER BY seq_in_index SEPARATOR ',') AS columns, \
                        CASE WHEN index_name = 'PRIMARY' THEN 1 ELSE 0 END AS is_primary \
                 FROM information_schema.statistics \
                 WHERE table_schema = DATABASE() \
                   AND table_name IN ({table_filter}) \
                   AND non_unique = 0 \
                   AND index_name NOT IN ('idx_files_unique_live_name', 'idx_folders_unique_live_name') \
                 GROUP BY table_name, index_name \
                 ORDER BY table_name, index_name"
            );
            let rows = db
                .query_all_raw(Statement::from_string(backend, sql))
                .await
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            rows.into_iter()
                .map(|row| {
                    Ok(UniqueIndex {
                        table: row
                            .try_get_by_index::<String>(0)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                        name: row
                            .try_get_by_index::<String>(1)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                        columns: row
                            .try_get_by_index::<String>(2)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?
                            .split(',')
                            .map(str::to_string)
                            .collect(),
                        is_primary: row
                            .try_get_by_index::<bool>(3)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                    })
                })
                .collect()
        }
        DbBackend::Sqlite => load_sqlite_unique_indexes(db, plans).await,
        _ => Err(AsterError::validation_error(
            "unsupported database backend for unique index verification",
        )),
    }
}

async fn load_sqlite_unique_indexes<C>(db: &C, plans: &[TablePlan]) -> Result<Vec<UniqueIndex>>
where
    C: ConnectionTrait,
{
    let mut indexes = Vec::new();
    let mut seen = BTreeSet::new();
    for plan in plans {
        if !plan.primary_key.is_empty() {
            indexes.push(UniqueIndex {
                table: plan.name.clone(),
                name: format!("{}_primary_key", plan.name),
                columns: plan.primary_key.clone(),
                is_primary: true,
            });
            seen.insert((plan.name.clone(), format!("{}_primary_key", plan.name)));
        }

        let sql = format!("PRAGMA index_list({})", quote_sqlite_literal(&plan.name));
        let rows = db
            .query_all_raw(Statement::from_string(DbBackend::Sqlite, sql))
            .await
            .map_err(|error| AsterError::database_operation(error.to_string()))?;

        for row in rows {
            let unique: i32 = row
                .try_get("", "unique")
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            if unique == 0 {
                continue;
            }

            let name: String = row
                .try_get("", "name")
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            if matches!(
                name.as_str(),
                "idx_files_unique_live_name" | "idx_folders_unique_live_name"
            ) {
                continue;
            }

            let origin: String = row
                .try_get("", "origin")
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            let info_sql = format!("PRAGMA index_info({})", quote_sqlite_literal(&name));
            let info_rows = db
                .query_all_raw(Statement::from_string(DbBackend::Sqlite, info_sql))
                .await
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            let mut columns = Vec::new();
            for info_row in info_rows {
                let column_name = info_row
                    .try_get::<Option<String>>("", "name")
                    .map_err(|error| AsterError::database_operation(error.to_string()))?;
                if let Some(column_name) = column_name {
                    columns.push(column_name);
                }
            }
            if columns.is_empty() || seen.contains(&(plan.name.clone(), name.clone())) {
                continue;
            }

            indexes.push(UniqueIndex {
                table: plan.name.clone(),
                name: name.clone(),
                columns,
                is_primary: origin == "pk",
            });
            seen.insert((plan.name.clone(), name));
        }
    }
    Ok(indexes)
}

async fn count_duplicate_groups<C>(
    db: &C,
    backend: DbBackend,
    table: &str,
    columns: &[String],
) -> Result<i64>
where
    C: ConnectionTrait,
{
    let table_ident = quote_ident(backend, table);
    let select_cols = columns
        .iter()
        .map(|column| quote_ident(backend, column))
        .collect::<Vec<_>>()
        .join(", ");
    let null_filter = columns
        .iter()
        .map(|column| format!("{} IS NOT NULL", quote_ident(backend, column)))
        .collect::<Vec<_>>()
        .join(" AND ");
    let where_clause = if null_filter.is_empty() {
        String::new()
    } else {
        format!(" WHERE {null_filter}")
    };
    let inner = format!(
        "SELECT 1 FROM {table_ident}{where_clause} GROUP BY {select_cols} HAVING COUNT(*) > 1"
    );
    let sql = format!("SELECT COUNT(*) FROM ({inner}) AS duplicate_groups");
    scalar_i64(db, backend, &sql).await
}

fn live_name_unique_checks() -> [(&'static str, &'static str, [&'static str; 5]); 2] {
    [
        (
            "files",
            "idx_files_unique_live_name",
            [
                "CASE WHEN team_id IS NULL THEN 0 ELSE 1 END",
                "CASE WHEN team_id IS NULL THEN user_id ELSE team_id END",
                "COALESCE(folder_id, 0)",
                "name",
                "CASE WHEN deleted_at IS NULL THEN 1 ELSE NULL END",
            ],
        ),
        (
            "folders",
            "idx_folders_unique_live_name",
            [
                "CASE WHEN team_id IS NULL THEN 0 ELSE 1 END",
                "CASE WHEN team_id IS NULL THEN user_id ELSE team_id END",
                "COALESCE(parent_id, 0)",
                "name",
                "CASE WHEN deleted_at IS NULL THEN 1 ELSE NULL END",
            ],
        ),
    ]
}

async fn count_expression_duplicates<C>(
    db: &C,
    backend: DbBackend,
    table: &str,
    expressions: [&str; 5],
) -> Result<i64>
where
    C: ConnectionTrait,
{
    let table_ident = quote_ident(backend, table);
    let group_by = expressions.join(", ");
    let inner = format!("SELECT 1 FROM {table_ident} GROUP BY {group_by} HAVING COUNT(*) > 1");
    let sql = format!("SELECT COUNT(*) FROM ({inner}) AS duplicate_groups");
    scalar_i64(db, backend, &sql).await
}

async fn load_foreign_keys<C>(
    db: &C,
    backend: DbBackend,
    plans: &[TablePlan],
) -> Result<Vec<ForeignKey>>
where
    C: ConnectionTrait,
{
    let table_filter = COPY_TABLE_ORDER
        .iter()
        .map(|table| quote_literal(table))
        .collect::<Vec<_>>()
        .join(", ");
    match backend {
        DbBackend::Postgres => {
            let sql = format!(
                "SELECT tc.table_name, tc.constraint_name, kcu.column_name, \
                        ccu.table_name AS referenced_table, ccu.column_name AS referenced_column \
                 FROM information_schema.table_constraints tc \
                 JOIN information_schema.key_column_usage kcu \
                   ON tc.constraint_name = kcu.constraint_name \
                  AND tc.table_schema = kcu.table_schema \
                 JOIN information_schema.constraint_column_usage ccu \
                   ON tc.constraint_name = ccu.constraint_name \
                  AND tc.table_schema = ccu.table_schema \
                 WHERE tc.constraint_type = 'FOREIGN KEY' \
                   AND tc.table_schema = current_schema() \
                   AND tc.table_name IN ({table_filter}) \
                 ORDER BY tc.table_name, tc.constraint_name, kcu.ordinal_position"
            );
            let rows = db
                .query_all_raw(Statement::from_string(backend, sql))
                .await
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            rows.into_iter()
                .map(|row| {
                    Ok(ForeignKey {
                        table: row
                            .try_get_by_index::<String>(0)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                        name: row
                            .try_get_by_index::<String>(1)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                        column: row
                            .try_get_by_index::<String>(2)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                        referenced_table: row
                            .try_get_by_index::<String>(3)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                        referenced_column: row
                            .try_get_by_index::<String>(4)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                    })
                })
                .collect()
        }
        DbBackend::MySql => {
            let sql = format!(
                "SELECT table_name, constraint_name, column_name, \
                        referenced_table_name, referenced_column_name \
                 FROM information_schema.key_column_usage \
                 WHERE table_schema = DATABASE() \
                   AND referenced_table_name IS NOT NULL \
                   AND table_name IN ({table_filter}) \
                 ORDER BY table_name, constraint_name, ordinal_position"
            );
            let rows = db
                .query_all_raw(Statement::from_string(backend, sql))
                .await
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            rows.into_iter()
                .map(|row| {
                    Ok(ForeignKey {
                        table: row
                            .try_get_by_index::<String>(0)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                        name: row
                            .try_get_by_index::<String>(1)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                        column: row
                            .try_get_by_index::<String>(2)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                        referenced_table: row
                            .try_get_by_index::<String>(3)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                        referenced_column: row
                            .try_get_by_index::<String>(4)
                            .map_err(|error| AsterError::database_operation(error.to_string()))?,
                    })
                })
                .collect()
        }
        DbBackend::Sqlite => load_sqlite_foreign_keys(db, plans).await,
        _ => Err(AsterError::validation_error(
            "unsupported database backend for foreign-key verification",
        )),
    }
}

async fn load_sqlite_foreign_keys<C>(db: &C, plans: &[TablePlan]) -> Result<Vec<ForeignKey>>
where
    C: ConnectionTrait,
{
    let mut foreign_keys = Vec::new();
    for plan in plans {
        let sql = format!(
            "PRAGMA foreign_key_list({})",
            quote_sqlite_literal(&plan.name)
        );
        let rows = db
            .query_all_raw(Statement::from_string(DbBackend::Sqlite, sql))
            .await
            .map_err(|error| AsterError::database_operation(error.to_string()))?;
        for row in rows {
            let id: i64 = row
                .try_get("", "id")
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            let referenced_table: String = row
                .try_get("", "table")
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            let column: String = row
                .try_get("", "from")
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            let referenced_column: String = row
                .try_get("", "to")
                .map_err(|error| AsterError::database_operation(error.to_string()))?;
            foreign_keys.push(ForeignKey {
                table: plan.name.clone(),
                name: format!("{}_fk_{}", plan.name, id),
                column,
                referenced_table,
                referenced_column,
            });
        }
    }
    Ok(foreign_keys)
}

async fn count_foreign_key_violations<C>(
    db: &C,
    backend: DbBackend,
    foreign_key: &ForeignKey,
) -> Result<i64>
where
    C: ConnectionTrait,
{
    let child_table = quote_ident(backend, &foreign_key.table);
    let child_column = quote_ident(backend, &foreign_key.column);
    let parent_table = quote_ident(backend, &foreign_key.referenced_table);
    let parent_column = quote_ident(backend, &foreign_key.referenced_column);
    let sql = format!(
        "SELECT COUNT(*) \
         FROM {child_table} child \
         LEFT JOIN {parent_table} parent \
           ON child.{child_column} = parent.{parent_column} \
         WHERE child.{child_column} IS NOT NULL \
           AND parent.{parent_column} IS NULL"
    );
    scalar_i64(db, backend, &sql).await
}

async fn ensure_checkpoint_table(target: &DatabaseConnection) -> Result<()> {
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
        .map_err(|error| AsterError::database_operation(error.to_string()))?;
    Ok(())
}

async fn initialize_checkpoint(
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
        .map_err(|error| AsterError::database_operation(error.to_string()))?
    else {
        return Ok(None);
    };

    Ok(Some(MigrationCheckpoint {
        migration_key: row
            .try_get_by_index::<String>(0)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
        source_database_url: redact_database_url(
            &row.try_get_by_index::<String>(1)
                .map_err(|error| AsterError::database_operation(error.to_string()))?,
        ),
        target_database_url: redact_database_url(
            &row.try_get_by_index::<String>(2)
                .map_err(|error| AsterError::database_operation(error.to_string()))?,
        ),
        mode: row
            .try_get_by_index::<String>(3)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
        status: row
            .try_get_by_index::<String>(4)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
        stage: row
            .try_get_by_index::<String>(5)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
        current_table: row
            .try_get_by_index::<Option<String>>(6)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
        current_table_index: row
            .try_get_by_index::<i64>(7)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
        current_table_offset: row
            .try_get_by_index::<i64>(8)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
        copied_rows: row
            .try_get_by_index::<i64>(9)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
        total_rows: row
            .try_get_by_index::<i64>(10)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
        plan_json: row
            .try_get_by_index::<String>(11)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
        result_json: row
            .try_get_by_index::<Option<String>>(12)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
        last_error: row
            .try_get_by_index::<Option<String>>(13)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
        heartbeat_at_ms: row
            .try_get_by_index::<i64>(14)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
        updated_at_ms: row
            .try_get_by_index::<i64>(15)
            .map_err(|error| AsterError::database_operation(error.to_string()))?,
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
        .map_err(|error| AsterError::database_operation(error.to_string()))?;
    Ok(())
}

async fn update_checkpoint<C>(db: &C, checkpoint: &MigrationCheckpoint) -> Result<()>
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
        .map_err(|error| AsterError::database_operation(error.to_string()))?;
    Ok(())
}

async fn mark_checkpoint_failed(
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

async fn count_rows<C>(db: &C, backend: DbBackend, table_name: &str) -> Result<i64>
where
    C: ConnectionTrait,
{
    scalar_i64(
        db,
        backend,
        &format!("SELECT COUNT(*) FROM {}", quote_ident(backend, table_name)),
    )
    .await
}

async fn scalar_i64<C>(db: &C, backend: DbBackend, sql: &str) -> Result<i64>
where
    C: ConnectionTrait,
{
    let row = db
        .query_one_raw(Statement::from_string(backend, sql))
        .await
        .map_err(|error| AsterError::database_operation(error.to_string()))?
        .ok_or_else(|| AsterError::database_operation(format!("query returned no rows: {sql}")))?;

    if let Ok(value) = row.try_get_by_index::<i64>(0) {
        return Ok(value);
    }
    if let Ok(value) = row.try_get_by_index::<i32>(0) {
        return Ok(i64::from(value));
    }
    if let Ok(value) = row.try_get_by_index::<bool>(0) {
        return Ok(if value { 1 } else { 0 });
    }

    Err(AsterError::database_operation(format!(
        "failed to decode scalar query result as integer: {sql}"
    )))
}

fn total_source_rows(plans: &[TablePlan]) -> i64 {
    plans.iter().map(|plan| plan.source_rows).sum()
}

fn join_strings(values: &[String]) -> String {
    values.join(", ")
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

fn copy_batch_size() -> Result<i64> {
    parse_positive_i64_env(COPY_BATCH_SIZE_ENV, DEFAULT_COPY_BATCH_SIZE)
}

fn fail_after_batches() -> Result<Option<i64>> {
    if std::env::var_os(FAIL_AFTER_BATCHES_ENV).is_none() {
        return Ok(None);
    }

    Ok(Some(parse_positive_i64_env(FAIL_AFTER_BATCHES_ENV, 0)?))
}

fn parse_positive_i64_env(name: &str, default_value: i64) -> Result<i64> {
    let Some(raw) = std::env::var_os(name) else {
        return Ok(default_value);
    };
    let raw = raw.to_string_lossy();
    let value = raw.parse::<i64>().map_err(|_| {
        AsterError::validation_error(format!("environment variable {name} must be an integer"))
    })?;
    if value <= 0 {
        return Err(AsterError::validation_error(format!(
            "environment variable {name} must be greater than zero"
        )));
    }
    Ok(value)
}

fn env_truthy(name: &str) -> bool {
    let Some(value) = std::env::var_os(name) else {
        return false;
    };
    matches!(
        value.to_string_lossy().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis() as i64
}

fn format_percent(current: i64, total: i64) -> String {
    if total <= 0 {
        return "0.0%".to_string();
    }
    format!("{:.1}%", (current as f64 / total as f64) * 100.0)
}

fn binding_kind_is_integer(kind: BindingKind) -> bool {
    matches!(kind, BindingKind::Int32 | BindingKind::Int64)
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_timestamp(value: &str) -> Option<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value).ok()
}

fn nullable_sql_string(value: Option<&str>) -> String {
    value
        .map(quote_literal)
        .unwrap_or_else(|| "NULL".to_string())
}

fn resume_message(checkpoint: &MigrationCheckpoint) -> String {
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

fn quote_ident(backend: DbBackend, ident: &str) -> String {
    match backend {
        DbBackend::MySql => format!("`{}`", ident.replace('`', "``")),
        DbBackend::Postgres | DbBackend::Sqlite => {
            format!("\"{}\"", ident.replace('"', "\"\""))
        }
        _ => format!("\"{}\"", ident.replace('"', "\"\"")),
    }
}

fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn quote_sqlite_literal(value: &str) -> String {
    quote_literal(value)
}

fn redact_database_url(database_url: &str) -> String {
    if database_url == "sqlite::memory:" {
        return database_url.to_string();
    }

    if database_url.starts_with("sqlite:") {
        return redact_sqlite_database_url(database_url);
    }

    let Some((scheme, rest)) = database_url.split_once("://") else {
        return database_url.to_string();
    };

    let Some((_authority, suffix)) = rest.rsplit_once('@') else {
        return database_url.to_string();
    };

    format!("{scheme}://***@{suffix}")
}

fn redact_sqlite_database_url(database_url: &str) -> String {
    let Some(path_and_query) = database_url.strip_prefix("sqlite://") else {
        return database_url.to_string();
    };
    let (path, query) = path_and_query
        .split_once('?')
        .map_or((path_and_query, None), |(path, query)| (path, Some(query)));
    let redacted_path = redact_sqlite_path(path);

    match query {
        Some(query) => format!("sqlite://{redacted_path}?{query}"),
        None => format!("sqlite://{redacted_path}"),
    }
}

fn redact_sqlite_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return "***".to_string();
    }

    let Some(file_name) = Path::new(trimmed)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
    else {
        return "***".to_string();
    };

    if path.starts_with('/') {
        format!("/.../{file_name}")
    } else {
        format!(".../{file_name}")
    }
}

#[cfg(test)]
mod tests {
    use super::redact_database_url;

    #[test]
    fn redact_database_url_masks_network_credentials() {
        assert_eq!(
            redact_database_url("postgres://postgres:postgres@127.0.0.1:5432/asterdrive"),
            "postgres://***@127.0.0.1:5432/asterdrive"
        );
        assert_eq!(
            redact_database_url("mysql://aster@db.internal:3306/asterdrive"),
            "mysql://***@db.internal:3306/asterdrive"
        );
    }

    #[test]
    fn redact_database_url_masks_sqlite_paths_but_preserves_filename() {
        assert_eq!(
            redact_database_url("sqlite:///Users/esap/Desktop/Github/AsterDrive/data/asterdrive.db?mode=rwc"),
            "sqlite:///.../asterdrive.db?mode=rwc"
        );
        assert_eq!(
            redact_database_url("sqlite://data/asterdrive.db?mode=rwc"),
            "sqlite://.../asterdrive.db?mode=rwc"
        );
        assert_eq!(redact_database_url("sqlite::memory:"), "sqlite::memory:");
    }
}
