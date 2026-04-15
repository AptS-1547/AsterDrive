use std::collections::HashSet;
use std::path::Path;

use crate::errors::{AsterError, MapAsterErr, Result};
use crate::services::integrity_service;
use clap::{Args, ValueEnum};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::Serialize;

use super::shared::{
    CliTerminalPalette, OutputFormat, ResolvedOutputFormat, connect_database, human_key,
    render_success_envelope,
};

#[derive(Debug, Clone, Args)]
pub struct DoctorArgs {
    #[arg(long, env = "ASTER_CLI_DATABASE_URL")]
    pub database_url: String,
    #[arg(long, env = "ASTER_CLI_DOCTOR_STRICT", default_value_t = false)]
    pub strict: bool,
    #[arg(long, env = "ASTER_CLI_DOCTOR_DEEP", default_value_t = false)]
    pub deep: bool,
    #[arg(long, env = "ASTER_CLI_DOCTOR_FIX", default_value_t = false)]
    pub fix: bool,
    #[arg(
        long = "scope",
        env = "ASTER_CLI_DOCTOR_SCOPE",
        value_enum,
        value_delimiter = ','
    )]
    pub scopes: Vec<DoctorDeepScope>,
    #[arg(long, env = "ASTER_CLI_DOCTOR_POLICY_ID")]
    pub policy_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum DoctorDeepScope {
    StorageUsage,
    BlobRefCounts,
    StorageObjects,
    FolderTree,
}

impl DoctorDeepScope {
    fn label(self) -> &'static str {
        match self {
            Self::StorageUsage => "storage_usage",
            Self::BlobRefCounts => "blob_ref_counts",
            Self::StorageObjects => "storage_objects",
            Self::FolderTree => "folder_tree",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorStatus {
    Ok,
    Warn,
    Fail,
}

impl DoctorStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Fail => "fail",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DoctorSummary {
    total: usize,
    ok: usize,
    warn: usize,
    fail: usize,
}

#[derive(Debug, Serialize)]
pub struct DoctorCheck {
    name: &'static str,
    label: &'static str,
    status: DoctorStatus,
    summary: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    details: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    strict: bool,
    deep: bool,
    fix: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    scopes: Vec<DoctorDeepScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    policy_id: Option<i64>,
    status: DoctorStatus,
    database_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    backend: Option<String>,
    summary: DoctorSummary,
    checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    fn new(
        args: &DoctorArgs,
        database_url: String,
        backend: Option<String>,
        deep: bool,
        scopes: Vec<DoctorDeepScope>,
        checks: Vec<DoctorCheck>,
    ) -> Self {
        let mut ok = 0;
        let mut warn = 0;
        let mut fail = 0;
        for check in &checks {
            match check.status {
                DoctorStatus::Ok => ok += 1,
                DoctorStatus::Warn => warn += 1,
                DoctorStatus::Fail => fail += 1,
            }
        }

        let status = if fail > 0 || (args.strict && warn > 0) {
            DoctorStatus::Fail
        } else if warn > 0 {
            DoctorStatus::Warn
        } else {
            DoctorStatus::Ok
        };

        Self {
            strict: args.strict,
            deep,
            fix: args.fix,
            scopes,
            policy_id: args.policy_id,
            status,
            database_url,
            backend,
            summary: DoctorSummary {
                total: checks.len(),
                ok,
                warn,
                fail,
            },
            checks,
        }
    }

    pub fn should_exit_nonzero(&self) -> bool {
        self.status == DoctorStatus::Fail
    }
}

fn effective_deep_scopes(args: &DoctorArgs) -> Vec<DoctorDeepScope> {
    if args.scopes.is_empty() {
        return vec![
            DoctorDeepScope::StorageUsage,
            DoctorDeepScope::BlobRefCounts,
            DoctorDeepScope::StorageObjects,
            DoctorDeepScope::FolderTree,
        ];
    }

    let mut deduped = Vec::new();
    for scope in &args.scopes {
        if !deduped.contains(scope) {
            deduped.push(*scope);
        }
    }
    deduped
}

fn doctor_scope_enabled(scopes: &[DoctorDeepScope], target: DoctorDeepScope) -> bool {
    scopes.contains(&target)
}

pub async fn execute_doctor_command(args: &DoctorArgs) -> DoctorReport {
    let redacted_database_url = redact_database_url(&args.database_url);
    let deep = args.deep || args.fix || !args.scopes.is_empty() || args.policy_id.is_some();
    let scopes = if deep {
        effective_deep_scopes(args)
    } else {
        Vec::new()
    };
    let mut backend = None;
    let mut checks = Vec::new();

    let db = match connect_database(&args.database_url).await {
        Ok(db) => {
            let db_backend = db.get_database_backend();
            let db_backend_name = backend_name(db_backend).to_string();
            backend = Some(db_backend_name.clone());
            checks.push(DoctorCheck {
                name: "database_connection",
                label: "Database connection",
                status: DoctorStatus::Ok,
                summary: format!("connected to {db_backend_name}"),
                details: vec![format!("database_url={redacted_database_url}")],
                suggestion: None,
            });
            Some((db, db_backend))
        }
        Err(err) => {
            checks.push(DoctorCheck {
                name: "database_connection",
                label: "Database connection",
                status: DoctorStatus::Fail,
                summary: "database connection failed".to_string(),
                details: vec![err.message().to_string()],
                suggestion: Some(
                    "Check --database-url, database availability, and access permissions."
                        .to_string(),
                ),
            });
            None
        }
    };

    let Some((db, db_backend)) = db else {
        return DoctorReport::new(args, redacted_database_url, backend, deep, scopes, checks);
    };

    let pending_migrations = match doctor_pending_migrations(&db, db_backend).await {
        Ok(pending) => {
            checks.push(if pending.is_empty() {
                DoctorCheck {
                    name: "database_migrations",
                    label: "Database migrations",
                    status: DoctorStatus::Ok,
                    summary: "no pending migrations".to_string(),
                    details: Vec::new(),
                    suggestion: None,
                }
            } else {
                DoctorCheck {
                    name: "database_migrations",
                    label: "Database migrations",
                    status: DoctorStatus::Warn,
                    summary: format!("{} pending migration(s)", pending.len()),
                    details: pending.clone(),
                    suggestion: Some(
                        "Apply pending migrations before running maintenance-oriented CLI commands."
                            .to_string(),
                    ),
                }
            });
            Some(pending)
        }
        Err(err) => {
            checks.push(DoctorCheck {
                name: "database_migrations",
                label: "Database migrations",
                status: DoctorStatus::Fail,
                summary: "failed to inspect migration history".to_string(),
                details: vec![err.message().to_string()],
                suggestion: Some(
                    "Check the seaql_migrations table and database permissions to ensure migration metadata is readable."
                        .to_string(),
                ),
            });
            None
        }
    };

    if db_backend == DbBackend::Sqlite {
        checks.push(match pending_migrations.as_ref() {
            Some(pending) => doctor_sqlite_search_check(&db, pending).await,
            None => DoctorCheck {
                name: "sqlite_search_acceleration",
                label: "SQLite search acceleration",
                status: DoctorStatus::Fail,
                summary: "failed to verify SQLite search acceleration".to_string(),
                details: vec!["migration status is unavailable".to_string()],
                suggestion: Some(
                    "Fix migration metadata access first, then rerun doctor to validate SQLite FTS5 trigram support."
                        .to_string(),
                ),
            },
        });
    }

    let runtime_config = crate::config::RuntimeConfig::new();
    let runtime_loaded = match runtime_config.reload(&db).await {
        Ok(()) => {
            checks.push(DoctorCheck {
                name: "runtime_config",
                label: "Runtime configuration",
                status: DoctorStatus::Ok,
                summary: "runtime config snapshot loaded".to_string(),
                details: Vec::new(),
                suggestion: None,
            });
            true
        }
        Err(err) => {
            checks.push(DoctorCheck {
                name: "runtime_config",
                label: "Runtime configuration",
                status: DoctorStatus::Fail,
                summary: "failed to load runtime config snapshot".to_string(),
                details: vec![err.message().to_string()],
                suggestion: Some(
                    "Check whether the system_config schema and stored values are complete."
                        .to_string(),
                ),
            });
            false
        }
    };

    if runtime_loaded {
        checks.push(doctor_public_site_url_check(&runtime_config));
        checks.push(doctor_mail_check(&runtime_config));
        checks.push(doctor_preview_apps_check(&runtime_config));
    }

    checks.push(doctor_storage_policy_check(&db).await);

    let mut effective_policy_id = args.policy_id;
    let mut policy_filter_valid = true;
    if deep && let Some(policy_id) = args.policy_id {
        match crate::db::repository::policy_repo::find_by_id(&db, policy_id).await {
            Ok(policy) => checks.push(DoctorCheck {
                name: "policy_filter",
                label: "Policy filter",
                status: DoctorStatus::Ok,
                summary: format!("scoped to storage policy #{} ({})", policy.id, policy.name),
                details: Vec::new(),
                suggestion: None,
            }),
            Err(err) => {
                checks.push(DoctorCheck {
                    name: "policy_filter",
                    label: "Policy filter",
                    status: DoctorStatus::Fail,
                    summary: format!("storage policy #{} does not exist", policy_id),
                    details: vec![err.message().to_string()],
                    suggestion: Some("Use a valid --policy-id or remove the filter.".to_string()),
                });
                effective_policy_id = None;
                policy_filter_valid = false;
            }
        }
    }

    if deep && doctor_scope_enabled(&scopes, DoctorDeepScope::StorageUsage) {
        checks.push(match doctor_storage_usage_check(&db, args.fix).await {
            Ok(check) => check,
            Err(err) => DoctorCheck {
                name: "storage_usage_consistency",
                label: "Storage usage counters",
                status: DoctorStatus::Fail,
                summary: "failed to audit storage usage counters".to_string(),
                details: vec![err.message().to_string()],
                suggestion: Some(
                    "Check whether the users, teams, files, and file_versions tables are complete and readable."
                        .to_string(),
                ),
            },
        });
    }

    if deep && doctor_scope_enabled(&scopes, DoctorDeepScope::BlobRefCounts) && policy_filter_valid
    {
        checks.push(
            match doctor_blob_ref_count_check(&db, args.fix, effective_policy_id).await {
                Ok(check) => check,
                Err(err) => DoctorCheck {
                    name: "blob_ref_counts",
                    label: "Blob reference counters",
                    status: DoctorStatus::Fail,
                    summary: "failed to audit blob reference counters".to_string(),
                    details: vec![err.message().to_string()],
                    suggestion: Some(
                        "Check whether the file_blobs, files, and file_versions tables are complete and readable."
                            .to_string(),
                    ),
                },
            },
        );
    }

    if deep && doctor_scope_enabled(&scopes, DoctorDeepScope::StorageObjects) && policy_filter_valid
    {
        match doctor_storage_scan_checks(&db, effective_policy_id).await {
            Ok(storage_checks) => checks.extend(storage_checks),
            Err(err) => checks.extend([
                DoctorCheck {
                    name: "tracked_blob_objects",
                    label: "Tracked blob objects",
                    status: DoctorStatus::Fail,
                    summary: "failed to scan storage objects".to_string(),
                    details: vec![err.message().to_string()],
                    suggestion: Some(
                        "Check storage policy configuration, driver permissions, and object storage connectivity."
                            .to_string(),
                    ),
                },
                DoctorCheck {
                    name: "untracked_storage_objects",
                    label: "Untracked storage objects",
                    status: DoctorStatus::Fail,
                    summary: "failed to scan storage objects".to_string(),
                    details: vec![err.message().to_string()],
                    suggestion: Some(
                        "Check storage policy configuration, driver permissions, and object storage connectivity."
                            .to_string(),
                    ),
                },
                DoctorCheck {
                    name: "thumbnail_objects",
                    label: "Thumbnail objects",
                    status: DoctorStatus::Fail,
                    summary: "failed to scan storage objects".to_string(),
                    details: vec![err.message().to_string()],
                    suggestion: Some(
                        "Check storage policy configuration, driver permissions, and object storage connectivity."
                            .to_string(),
                    ),
                },
            ]),
        }
    }

    if deep && doctor_scope_enabled(&scopes, DoctorDeepScope::FolderTree) {
        checks.push(match doctor_folder_tree_check(&db).await {
            Ok(check) => check,
            Err(err) => DoctorCheck {
                name: "folder_tree_integrity",
                label: "Folder tree integrity",
                status: DoctorStatus::Fail,
                summary: "failed to audit folder tree".to_string(),
                details: vec![err.message().to_string()],
                suggestion: Some(
                    "Check whether the folders table is complete and whether parent_id relationships are readable."
                        .to_string(),
                ),
            },
        });
    }

    DoctorReport::new(args, redacted_database_url, backend, deep, scopes, checks)
}

async fn doctor_sqlite_search_check(
    db: &sea_orm::DatabaseConnection,
    pending_migrations: &[String],
) -> DoctorCheck {
    match crate::db::sqlite_search::inspect_sqlite_search_status(db).await {
        Ok(Some(status)) if status.is_ready() => DoctorCheck {
            name: "sqlite_search_acceleration",
            label: "SQLite search acceleration",
            status: DoctorStatus::Ok,
            summary: "FTS5 trigram search acceleration ready".to_string(),
            details: status.detail_lines(),
            suggestion: None,
        },
        Ok(Some(status))
            if pending_migrations
                .iter()
                .any(|name| {
                    crate::db::sqlite_search::SQLITE_SEARCH_MIGRATION_NAMES
                        .contains(&name.as_str())
                }) =>
        {
            DoctorCheck {
                name: "sqlite_search_acceleration",
                label: "SQLite search acceleration",
                status: DoctorStatus::Warn,
                summary: "SQLite search acceleration migration pending".to_string(),
                details: status.detail_lines(),
                suggestion: Some(
                    "Apply pending migrations on a SQLite build that includes FTS5 with the trigram tokenizer."
                        .to_string(),
                ),
            }
        }
        Ok(Some(status)) if !status.probe_supported() => DoctorCheck {
            name: "sqlite_search_acceleration",
            label: "SQLite search acceleration",
            status: DoctorStatus::Fail,
            summary: "SQLite build lacks FTS5 trigram search support".to_string(),
            details: status.detail_lines(),
            suggestion: Some(
                "Use a SQLite build with FTS5 + trigram tokenizer support, or switch the deployment to PostgreSQL / MySQL."
                    .to_string(),
            ),
        },
        Ok(Some(status)) => DoctorCheck {
            name: "sqlite_search_acceleration",
            label: "SQLite search acceleration",
            status: DoctorStatus::Fail,
                summary: "SQLite search acceleration objects are missing".to_string(),
                details: status.detail_lines(),
                suggestion: Some(
                "Apply the latest migrations and restore the files_name_fts / folders_name_fts / users_search_fts / teams_search_fts objects if they were removed manually."
                    .to_string(),
                ),
            },
        Ok(None) => DoctorCheck {
            name: "sqlite_search_acceleration",
            label: "SQLite search acceleration",
            status: DoctorStatus::Ok,
            summary: "not applicable".to_string(),
            details: Vec::new(),
            suggestion: None,
        },
        Err(err) => DoctorCheck {
            name: "sqlite_search_acceleration",
            label: "SQLite search acceleration",
            status: DoctorStatus::Fail,
            summary: "failed to verify SQLite search acceleration".to_string(),
            details: vec![err.message().to_string()],
            suggestion: Some(
                "Check SQLite metadata access, then rerun doctor to validate FTS5 trigram support and search objects."
                    .to_string(),
            ),
        },
    }
}

pub fn render_doctor_success(format: OutputFormat, report: &DoctorReport) -> String {
    match format.resolve() {
        ResolvedOutputFormat::Json => render_success_envelope(report, false),
        ResolvedOutputFormat::PrettyJson => render_success_envelope(report, true),
        ResolvedOutputFormat::Human => render_doctor_human(report),
    }
}

fn render_doctor_human(report: &DoctorReport) -> String {
    let palette = CliTerminalPalette::stdout();
    let mut lines = vec![
        palette.title("System doctor"),
        palette.dim("--------------------------------------------------"),
        format!(
            "{} {}",
            human_key("Database", &palette),
            report.database_url
        ),
        format!(
            "{} {}",
            human_key("Backend", &palette),
            report.backend.as_deref().unwrap_or("unknown")
        ),
        format!(
            "{} {}",
            human_key("Mode", &palette),
            doctor_mode_label(report)
        ),
        format!(
            "{} {}",
            human_key("Scope", &palette),
            if report.deep {
                doctor_scope_label(report)
            } else {
                "default".to_string()
            }
        ),
        format!(
            "{} {} {}",
            human_key("Status", &palette),
            palette.status_badge(report.status.as_str()),
            doctor_status_label(report.status)
        ),
        format!(
            "{} {} total, {} ok, {} warn, {} fail",
            human_key("Checks", &palette),
            report.summary.total,
            report.summary.ok,
            report.summary.warn,
            report.summary.fail
        ),
    ];

    if report.checks.is_empty() {
        lines.push(String::new());
        lines.push(palette.dim("No checks were executed."));
        return lines.join("\n");
    }

    lines.push(String::new());
    lines.push(palette.label("Checks:"));
    for check in &report.checks {
        lines.push(format!(
            "  {} {}",
            palette.status_badge(check.status.as_str()),
            check.label
        ));
        lines.push(format!("    {}", check.summary));
        for detail in &check.details {
            lines.push(format!("    {}", palette.dim(detail)));
        }
        if let Some(suggestion) = &check.suggestion {
            lines.push(format!(
                "    {} {}",
                palette.label("hint:"),
                palette.accent(suggestion)
            ));
        }
    }

    lines.join("\n")
}

fn doctor_mode_label(report: &DoctorReport) -> String {
    let mut parts = Vec::new();
    parts.push(if report.strict { "strict" } else { "standard" });
    if report.deep {
        parts.push("deep");
    }
    if report.fix {
        parts.push("fix");
    }
    parts.join(" + ")
}

fn doctor_scope_label(report: &DoctorReport) -> String {
    let mut label = if report.scopes.is_empty() {
        "default".to_string()
    } else {
        report
            .scopes
            .iter()
            .map(|scope| scope.label())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if let Some(policy_id) = report.policy_id {
        label.push_str(&format!(" | policy_id={policy_id}"));
    }
    label
}

async fn doctor_storage_usage_check(
    db: &sea_orm::DatabaseConnection,
    fix: bool,
) -> Result<DoctorCheck> {
    let mut drifts = integrity_service::audit_storage_usage(db).await?;
    let detected = drifts.len();
    let mut fixed = 0usize;

    if fix && !drifts.is_empty() {
        integrity_service::fix_storage_usage_drifts(db, &drifts).await?;
        fixed = drifts.len();
        drifts = integrity_service::audit_storage_usage(db).await?;
    }

    if drifts.is_empty() {
        let summary = if fixed > 0 {
            format!("fixed {fixed} storage usage mismatch(es)")
        } else {
            "storage usage counters match logical file sizes".to_string()
        };
        let mut details = Vec::new();
        if detected > 0 {
            details.push(format!("detected_before_fix={detected}"));
        }
        return Ok(DoctorCheck {
            name: "storage_usage_consistency",
            label: "Storage usage counters",
            status: DoctorStatus::Ok,
            summary,
            details,
            suggestion: None,
        });
    }

    let details = drifts
        .into_iter()
        .map(|drift| {
            format!(
                "{}#{} recorded={} actual={} delta={}",
                match drift.owner_kind {
                    integrity_service::StorageOwnerKind::User => "user",
                    integrity_service::StorageOwnerKind::Team => "team",
                },
                drift.owner_id,
                drift.recorded_bytes,
                drift.actual_bytes,
                drift.delta_bytes
            )
        })
        .collect();

    Ok(DoctorCheck {
        name: "storage_usage_consistency",
        label: "Storage usage counters",
        status: DoctorStatus::Warn,
        summary: format!("{} storage usage mismatch(es)", detected.max(fixed)),
        details,
        suggestion: Some(
            "Run doctor --deep --fix to write back users.storage_used and teams.storage_used."
                .to_string(),
        ),
    })
}

async fn doctor_blob_ref_count_check(
    db: &sea_orm::DatabaseConnection,
    fix: bool,
    policy_id: Option<i64>,
) -> Result<DoctorCheck> {
    let mut drifts = integrity_service::audit_blob_ref_counts(db, policy_id).await?;
    let detected = drifts.len();
    let mut fixed = 0usize;

    if fix && !drifts.is_empty() {
        integrity_service::fix_blob_ref_count_drifts(db, &drifts).await?;
        fixed = drifts.len();
        drifts = integrity_service::audit_blob_ref_counts(db, policy_id).await?;
    }

    if drifts.is_empty() {
        let summary = if fixed > 0 {
            format!("fixed {fixed} blob ref_count mismatch(es)")
        } else {
            if let Some(policy_id) = policy_id {
                format!("blob ref_count values match file references for policy #{policy_id}")
            } else {
                "blob ref_count values match file references".to_string()
            }
        };
        let mut details = Vec::new();
        if detected > 0 {
            details.push(format!("detected_before_fix={detected}"));
        }
        return Ok(DoctorCheck {
            name: "blob_ref_counts",
            label: "Blob reference counters",
            status: DoctorStatus::Ok,
            summary,
            details,
            suggestion: None,
        });
    }

    let details = drifts
        .into_iter()
        .map(|drift| {
            format!(
                "blob#{} recorded={} actual={} policy_id={} path={}",
                drift.blob_id,
                drift.recorded_ref_count,
                drift.actual_ref_count,
                drift.policy_id,
                drift.storage_path
            )
        })
        .collect();

    Ok(DoctorCheck {
        name: "blob_ref_counts",
        label: "Blob reference counters",
        status: DoctorStatus::Warn,
        summary: match policy_id {
            Some(policy_id) => format!(
                "{} blob ref_count mismatch(es) for policy #{}",
                detected.max(fixed),
                policy_id
            ),
            None => format!("{} blob ref_count mismatch(es)", detected.max(fixed)),
        },
        details,
        suggestion: Some("Run doctor --deep --fix to write back file_blobs.ref_count.".to_string()),
    })
}

async fn doctor_storage_scan_checks(
    db: &sea_orm::DatabaseConnection,
    policy_id: Option<i64>,
) -> Result<Vec<DoctorCheck>> {
    let driver_registry = crate::storage::DriverRegistry::new();
    let report = integrity_service::audit_storage_objects(db, &driver_registry, policy_id).await?;

    let scan_meta = vec![
        format!("policies={}", report.scanned_policies),
        format!("objects={}", report.scanned_objects),
        format!("ignored_paths={}", report.ignored_paths),
    ];

    let tracked_blob_check = if report.missing_blob_objects.is_empty() {
        DoctorCheck {
            name: "tracked_blob_objects",
            label: "Tracked blob objects",
            status: DoctorStatus::Ok,
            summary: match policy_id {
                Some(policy_id) => {
                    format!("all tracked blobs exist in storage for policy #{policy_id}")
                }
                None => "all tracked blobs exist in storage".to_string(),
            },
            details: scan_meta.clone(),
            suggestion: None,
        }
    } else {
        let mut details = scan_meta.clone();
        details.extend(
            report
                .missing_blob_objects
                .iter()
                .map(|issue| match issue.blob_id {
                    Some(blob_id) => format!(
                        "blob#{} policy_id={} missing path={}",
                        blob_id, issue.policy_id, issue.path
                    ),
                    None => format!("policy_id={} missing path={}", issue.policy_id, issue.path),
                }),
        );
        DoctorCheck {
            name: "tracked_blob_objects",
            label: "Tracked blob objects",
            status: DoctorStatus::Fail,
            summary: match policy_id {
                Some(policy_id) => format!(
                    "{} tracked blob object(s) are missing from storage for policy #{}",
                    report.missing_blob_objects.len(),
                    policy_id
                ),
                None => format!(
                    "{} tracked blob object(s) are missing from storage",
                    report.missing_blob_objects.len()
                ),
            },
            details,
            suggestion: Some(if report.scanned_objects == 0 {
                "No storage objects were listed. Check the storage policy base path / bucket / prefix first; for local policies, relative base_path values are resolved from the current working directory.".to_string()
            } else {
                "Check for missing objects in the underlying storage, bad migrations, or manual file deletion.".to_string()
            }),
        }
    };

    let untracked_storage_check = if report.untracked_objects.is_empty() {
        DoctorCheck {
            name: "untracked_storage_objects",
            label: "Untracked storage objects",
            status: DoctorStatus::Ok,
            summary: match policy_id {
                Some(policy_id) => {
                    format!("no extra storage objects were found for policy #{policy_id}")
                }
                None => "no extra storage objects were found".to_string(),
            },
            details: scan_meta.clone(),
            suggestion: None,
        }
    } else {
        let mut details = scan_meta.clone();
        details.extend(report.untracked_objects.iter().map(|issue| {
            format!(
                "policy_id={} untracked path={}",
                issue.policy_id, issue.path
            )
        }));
        DoctorCheck {
            name: "untracked_storage_objects",
            label: "Untracked storage objects",
            status: DoctorStatus::Warn,
            summary: match policy_id {
                Some(policy_id) => format!(
                    "{} untracked storage object(s) found for policy #{}",
                    report.untracked_objects.len(),
                    policy_id
                ),
                None => format!(
                    "{} untracked storage object(s) found",
                    report.untracked_objects.len()
                ),
            },
            details,
            suggestion: Some(
                "Confirm whether these are leftover temporary objects; clean them up manually or restore metadata if needed."
                    .to_string(),
            ),
        }
    };

    let thumbnail_check = if report.orphan_thumbnails.is_empty() {
        DoctorCheck {
            name: "thumbnail_objects",
            label: "Thumbnail objects",
            status: DoctorStatus::Ok,
            summary: match policy_id {
                Some(policy_id) => {
                    format!("thumbnail objects all map to known blobs for policy #{policy_id}")
                }
                None => "thumbnail objects all map to known blobs".to_string(),
            },
            details: scan_meta,
            suggestion: None,
        }
    } else {
        let mut details = vec![
            format!("policies={}", report.scanned_policies),
            format!("objects={}", report.scanned_objects),
            format!("ignored_paths={}", report.ignored_paths),
        ];
        details.extend(report.orphan_thumbnails.iter().map(|issue| {
            format!(
                "policy_id={} orphan_thumbnail={}",
                issue.policy_id, issue.path
            )
        }));
        DoctorCheck {
            name: "thumbnail_objects",
            label: "Thumbnail objects",
            status: DoctorStatus::Warn,
            summary: match policy_id {
                Some(policy_id) => format!(
                    "{} orphan thumbnail object(s) found for policy #{}",
                    report.orphan_thumbnails.len(),
                    policy_id
                ),
                None => format!(
                    "{} orphan thumbnail object(s) found",
                    report.orphan_thumbnails.len()
                ),
            },
            details,
            suggestion: Some(
                "These thumbnails are no longer referenced; remove them manually once confirmed unused."
                    .to_string(),
            ),
        }
    };

    Ok(vec![
        tracked_blob_check,
        untracked_storage_check,
        thumbnail_check,
    ])
}

async fn doctor_folder_tree_check(db: &sea_orm::DatabaseConnection) -> Result<DoctorCheck> {
    let issues = integrity_service::audit_folder_tree(db).await?;
    if issues.is_empty() {
        return Ok(DoctorCheck {
            name: "folder_tree_integrity",
            label: "Folder tree integrity",
            status: DoctorStatus::Ok,
            summary: "folder parent chains are internally consistent".to_string(),
            details: Vec::new(),
            suggestion: None,
        });
    }

    let has_cycle = issues
        .iter()
        .any(|issue| issue.kind == integrity_service::FolderTreeIssueKind::Cycle);
    let details = issues
        .into_iter()
        .map(|issue| {
            format!(
                "{} folder#{} {}",
                match issue.kind {
                    integrity_service::FolderTreeIssueKind::MissingParent => "missing_parent",
                    integrity_service::FolderTreeIssueKind::CrossScopeParent =>
                        "cross_scope_parent",
                    integrity_service::FolderTreeIssueKind::Cycle => "cycle",
                },
                issue.folder_id,
                issue.detail
            )
        })
        .collect();

    Ok(DoctorCheck {
        name: "folder_tree_integrity",
        label: "Folder tree integrity",
        status: DoctorStatus::Fail,
        summary: if has_cycle {
            "folder tree contains cycles or invalid parent references".to_string()
        } else {
            "folder tree contains invalid parent references".to_string()
        },
        details,
        suggestion: Some(
            "Fix dangling parent_id values or folder cycles before continuing with bulk move or delete operations."
                .to_string(),
        ),
    })
}

fn doctor_public_site_url_check(runtime_config: &crate::config::RuntimeConfig) -> DoctorCheck {
    let Some(raw_value) = runtime_config.get(crate::config::site_url::PUBLIC_SITE_URL_KEY) else {
        return DoctorCheck {
            name: "public_site_url",
            label: "Public site URL",
            status: DoctorStatus::Warn,
            summary: "public_site_url is not configured".to_string(),
            details: vec![
                "share, preview, and callback URLs will not have a stable public origin"
                    .to_string(),
            ],
            suggestion: Some(
                "Set config public_site_url to an externally reachable HTTP(S) origin.".to_string(),
            ),
        };
    };

    if raw_value.trim().is_empty() {
        return DoctorCheck {
            name: "public_site_url",
            label: "Public site URL",
            status: DoctorStatus::Warn,
            summary: "public_site_url is empty".to_string(),
            details: vec![
                "share, preview, and callback URLs will not have a stable public origin"
                    .to_string(),
            ],
            suggestion: Some(
                "Set config public_site_url to an externally reachable HTTP(S) origin.".to_string(),
            ),
        };
    }

    match crate::config::site_url::normalize_public_site_url_config_value(&raw_value) {
        Ok(normalized) => {
            if normalized.starts_with("http://") {
                return DoctorCheck {
                    name: "public_site_url",
                    label: "Public site URL",
                    status: DoctorStatus::Warn,
                    summary: "public_site_url uses insecure HTTP".to_string(),
                    details: vec![
                        format!("configured={normalized}"),
                        "production deployments should terminate TLS at a reverse proxy"
                            .to_string(),
                    ],
                    suggestion: Some(
                        "Put the site behind an HTTPS reverse proxy and change public_site_url to an https:// origin."
                            .to_string(),
                    ),
                };
            }

            DoctorCheck {
                name: "public_site_url",
                label: "Public site URL",
                status: DoctorStatus::Ok,
                summary: format!("configured as {normalized}"),
                details: Vec::new(),
                suggestion: None,
            }
        }
        Err(err) => DoctorCheck {
            name: "public_site_url",
            label: "Public site URL",
            status: DoctorStatus::Fail,
            summary: "public_site_url is invalid".to_string(),
            details: vec![err.message().to_string()],
            suggestion: Some(
                "Use a plain origin such as https://drive.example.com, without a path or non-HTTP(S) scheme."
                    .to_string(),
            ),
        },
    }
}

fn doctor_mail_check(runtime_config: &crate::config::RuntimeConfig) -> DoctorCheck {
    let settings = crate::config::mail::RuntimeMailSettings::from_runtime_config(runtime_config);
    let mut details = vec![
        format!(
            "smtp_host={}",
            non_empty_or_placeholder(&settings.smtp_host)
        ),
        format!("smtp_port={}", settings.smtp_port),
        format!(
            "from_address={}",
            non_empty_or_placeholder(&settings.from_address)
        ),
        format!(
            "auth={}",
            if settings.smtp_username.trim().is_empty() {
                "disabled"
            } else {
                "enabled"
            }
        ),
        format!(
            "transport_security={}",
            if settings.encryption_enabled {
                "enabled"
            } else {
                "disabled"
            }
        ),
    ];

    if settings.smtp_username.trim().is_empty() ^ settings.smtp_password.trim().is_empty() {
        details.push(
            "mail_smtp_username and mail_smtp_password must both be set or both be empty"
                .to_string(),
        );
        return DoctorCheck {
            name: "mail_configuration",
            label: "Mail configuration",
            status: DoctorStatus::Fail,
            summary: "SMTP authentication is only partially configured".to_string(),
            details,
            suggestion: Some(
                "Set both mail_smtp_username and mail_smtp_password together, or leave both empty."
                    .to_string(),
            ),
        };
    }

    if !settings.is_configured() {
        let mut missing = Vec::new();
        if settings.smtp_host.trim().is_empty() {
            missing.push("mail_smtp_host");
        }
        if settings.from_address.trim().is_empty() {
            missing.push("mail_from_address");
        }
        details.push(format!("missing={}", missing.join(", ")));
        return DoctorCheck {
            name: "mail_configuration",
            label: "Mail configuration",
            status: DoctorStatus::Warn,
            summary: "mail delivery is not fully configured".to_string(),
            details,
            suggestion: Some(
                "At minimum, set mail_smtp_host and mail_from_address to make mail delivery usable."
                    .to_string(),
            ),
        };
    }

    DoctorCheck {
        name: "mail_configuration",
        label: "Mail configuration",
        status: DoctorStatus::Ok,
        summary: "mail delivery settings are configured".to_string(),
        details,
        suggestion: None,
    }
}

fn doctor_preview_apps_check(runtime_config: &crate::config::RuntimeConfig) -> DoctorCheck {
    let raw = runtime_config
        .get(crate::services::preview_app_service::PREVIEW_APPS_CONFIG_KEY)
        .unwrap_or_else(crate::services::preview_app_service::default_public_preview_apps_json);

    let normalized =
        match crate::services::preview_app_service::normalize_public_preview_apps_config_value(&raw)
        {
            Ok(normalized) => normalized,
            Err(err) => {
                return DoctorCheck {
                    name: "preview_apps",
                    label: "Preview app registry",
                    status: DoctorStatus::Fail,
                    summary: "preview app registry is invalid".to_string(),
                    details: vec![err.message().to_string()],
                    suggestion: Some(
                        "Fix frontend_preview_apps_json or restore the default preview app configuration."
                            .to_string(),
                    ),
                };
            }
        };

    let parsed: crate::services::preview_app_service::PublicPreviewAppsConfig =
        match serde_json::from_str(&normalized) {
            Ok(parsed) => parsed,
            Err(err) => {
                return DoctorCheck {
                    name: "preview_apps",
                    label: "Preview app registry",
                    status: DoctorStatus::Fail,
                    summary: "preview app registry could not be parsed".to_string(),
                    details: vec![err.to_string()],
                    suggestion: Some(
                        "Check whether frontend_preview_apps_json was edited into an invalid state; restore the default value if needed."
                            .to_string(),
                    ),
                };
            }
        };

    let total_apps = parsed.apps.len();
    let enabled_apps = parsed.apps.iter().filter(|app| app.enabled).count();
    let wopi_apps = parsed
        .apps
        .iter()
        .filter(|app| {
            app.enabled
                && app.provider == crate::services::preview_app_service::PreviewAppProvider::Wopi
        })
        .count();
    let details = vec![
        format!("apps={total_apps}"),
        format!("enabled={enabled_apps}"),
        format!("wopi_enabled={wopi_apps}"),
    ];

    if wopi_apps > 0
        && runtime_config
            .get(crate::config::site_url::PUBLIC_SITE_URL_KEY)
            .is_none_or(|value| value.trim().is_empty())
    {
        return DoctorCheck {
            name: "preview_apps",
            label: "Preview app registry",
            status: DoctorStatus::Warn,
            summary: "WOPI preview apps are configured but public_site_url is empty".to_string(),
            details,
            suggestion: Some(
                "Set public_site_url or disable WOPI preview apps to avoid generating unusable preview entry points."
                    .to_string(),
            ),
        };
    }

    DoctorCheck {
        name: "preview_apps",
        label: "Preview app registry",
        status: DoctorStatus::Ok,
        summary: "preview app registry is valid".to_string(),
        details,
        suggestion: None,
    }
}

async fn doctor_storage_policy_check(db: &sea_orm::DatabaseConnection) -> DoctorCheck {
    let policies = match crate::db::repository::policy_repo::find_all(db).await {
        Ok(policies) => policies,
        Err(err) => {
            return DoctorCheck {
                name: "storage_policies",
                label: "Storage policies",
                status: DoctorStatus::Fail,
                summary: "failed to load storage policies".to_string(),
                details: vec![err.message().to_string()],
                suggestion: Some(
                    "Ensure database migrations are complete and the storage_policies table is accessible."
                        .to_string(),
                ),
            };
        }
    };
    let groups = match crate::db::repository::policy_group_repo::find_all_groups(db).await {
        Ok(groups) => groups,
        Err(err) => {
            return DoctorCheck {
                name: "storage_policies",
                label: "Storage policies",
                status: DoctorStatus::Fail,
                summary: "failed to load storage policy groups".to_string(),
                details: vec![err.message().to_string()],
                suggestion: Some(
                    "Ensure database migrations are complete and the storage_policy_groups table is accessible."
                        .to_string(),
                ),
            };
        }
    };

    let snapshot = crate::storage::PolicySnapshot::new();
    if let Err(err) = snapshot.reload(db).await {
        return DoctorCheck {
            name: "storage_policies",
            label: "Storage policies",
            status: DoctorStatus::Fail,
            summary: "failed to build storage policy snapshot".to_string(),
            details: vec![err.message().to_string()],
            suggestion: Some(
                "Check whether policies, policy groups, and user policy group assignments are consistent."
                    .to_string(),
            ),
        };
    }

    let default_policy = policies.iter().find(|policy| policy.is_default);
    let default_group = groups.iter().find(|group| group.is_default);
    let mut details = vec![
        format!("policies={}", policies.len()),
        format!("groups={}", groups.len()),
    ];
    let mut problems = Vec::new();

    if policies.is_empty() {
        problems.push("no storage policies found".to_string());
    }
    if let Some(policy) = default_policy {
        details.push(format!("default_policy={}", policy.name));
    } else {
        problems.push("no default storage policy found".to_string());
    }
    if let Some(group) = default_group {
        details.push(format!("default_group={}", group.name));
    } else {
        problems.push("no default storage policy group found".to_string());
    }
    if snapshot.system_default_policy().is_none() {
        problems.push("policy snapshot has no system default policy".to_string());
    }
    if snapshot.system_default_policy_group().is_none() {
        problems.push("policy snapshot has no system default group".to_string());
    }

    if problems.is_empty() {
        DoctorCheck {
            name: "storage_policies",
            label: "Storage policies",
            status: DoctorStatus::Ok,
            summary: "storage policy defaults are ready".to_string(),
            details,
            suggestion: None,
        }
    } else {
        details.extend(problems);
        DoctorCheck {
            name: "storage_policies",
            label: "Storage policies",
            status: DoctorStatus::Fail,
            summary: "storage policy setup is incomplete".to_string(),
            details,
            suggestion: Some(
                "Start the server once or seed the default storage policy and policy group data manually."
                    .to_string(),
            ),
        }
    }
}

fn doctor_status_label(status: DoctorStatus) -> &'static str {
    match status {
        DoctorStatus::Ok => "ready",
        DoctorStatus::Warn => "attention",
        DoctorStatus::Fail => "failed",
    }
}

fn non_empty_or_placeholder(value: &str) -> &str {
    if value.trim().is_empty() {
        "<empty>"
    } else {
        value
    }
}

fn backend_name(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::MySql => "mysql",
        DbBackend::Postgres => "postgres",
        DbBackend::Sqlite => "sqlite",
        _ => "unknown",
    }
}

fn doctor_migration_names() -> Vec<String> {
    Migrator::migrations()
        .into_iter()
        .map(|migration| migration.name().to_string())
        .collect()
}

async fn doctor_pending_migrations<C>(db: &C, backend: DbBackend) -> Result<Vec<String>>
where
    C: ConnectionTrait,
{
    let expected = doctor_migration_names();
    let applied = doctor_applied_migrations(db, backend).await?;
    let applied_lookup: HashSet<&str> = applied.iter().map(String::as_str).collect();
    let unknown_applied: Vec<String> = applied
        .iter()
        .filter(|name| !expected.iter().any(|expected_name| expected_name == *name))
        .cloned()
        .collect();
    if !unknown_applied.is_empty() {
        return Err(AsterError::validation_error(format!(
            "database contains unknown migration versions: {}",
            unknown_applied.join(", ")
        )));
    }

    Ok(expected
        .iter()
        .filter(|name| !applied_lookup.contains(name.as_str()))
        .cloned()
        .collect())
}

async fn doctor_applied_migrations<C>(db: &C, backend: DbBackend) -> Result<Vec<String>>
where
    C: ConnectionTrait,
{
    if !doctor_table_exists(db, backend, "seaql_migrations").await? {
        return Ok(Vec::new());
    }

    let sql = format!(
        "SELECT {} FROM {} ORDER BY {}",
        doctor_quote_ident(backend, "version"),
        doctor_quote_ident(backend, "seaql_migrations"),
        doctor_quote_ident(backend, "version")
    );
    let rows = db
        .query_all_raw(Statement::from_string(backend, sql))
        .await
        .map_aster_err(AsterError::database_operation)?;

    rows.into_iter()
        .map(|row| {
            row.try_get_by_index::<String>(0)
                .map_aster_err(AsterError::database_operation)
        })
        .collect()
}

async fn doctor_table_exists<C>(db: &C, backend: DbBackend, table_name: &str) -> Result<bool>
where
    C: ConnectionTrait,
{
    let sql = match backend {
        DbBackend::Sqlite => format!(
            "SELECT CASE WHEN EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = {}) THEN 1 ELSE 0 END",
            doctor_quote_literal(table_name)
        ),
        DbBackend::Postgres => format!(
            "SELECT CASE WHEN EXISTS(SELECT 1 FROM information_schema.tables \
             WHERE table_schema = current_schema() AND table_name = {}) THEN 1 ELSE 0 END",
            doctor_quote_literal(table_name)
        ),
        DbBackend::MySql => format!(
            "SELECT CASE WHEN EXISTS(SELECT 1 FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_name = {}) THEN 1 ELSE 0 END",
            doctor_quote_literal(table_name)
        ),
        _ => {
            return Err(AsterError::validation_error(
                "unsupported database backend for table existence checks",
            ));
        }
    };

    let row = db
        .query_one_raw(Statement::from_string(backend, sql))
        .await
        .map_aster_err(AsterError::database_operation)?
        .ok_or_else(|| AsterError::database_operation("table existence query returned no rows"))?;
    let exists = row
        .try_get_by_index::<i64>(0)
        .map_aster_err(AsterError::database_operation)?;
    Ok(exists != 0)
}

fn doctor_quote_ident(backend: DbBackend, ident: &str) -> String {
    match backend {
        DbBackend::MySql => format!("`{}`", ident.replace('`', "``")),
        DbBackend::Postgres | DbBackend::Sqlite => {
            format!("\"{}\"", ident.replace('"', "\"\""))
        }
        _ => ident.to_string(),
    }
}

fn doctor_quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
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

    if !rest.contains('@') {
        return database_url.to_string();
    }

    let authority_and_path = rest.split_once('@').map(|(_, tail)| tail).unwrap_or(rest);
    format!("{scheme}://***@{authority_and_path}")
}

fn redact_sqlite_database_url(database_url: &str) -> String {
    let Some(path_and_query) = database_url.strip_prefix("sqlite://") else {
        return database_url.to_string();
    };
    let (path, query) = path_and_query
        .split_once('?')
        .map_or((path_and_query, None), |(path, query)| (path, Some(query)));

    let redacted_path = if path == ":memory:" {
        path.to_string()
    } else {
        let filename = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path);
        if path.starts_with('/') {
            format!("/.../{filename}")
        } else {
            format!(".../{filename}")
        }
    };

    match query {
        Some(query) => format!("sqlite://{redacted_path}?{query}"),
        None => format!("sqlite://{redacted_path}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{DoctorStatus, doctor_public_site_url_check};
    use crate::config::RuntimeConfig;
    use crate::config::site_url::PUBLIC_SITE_URL_KEY;
    use crate::entities::system_config;
    use chrono::Utc;

    fn config_model(key: &str, value: &str) -> system_config::Model {
        system_config::Model {
            id: 1,
            key: key.to_string(),
            value: value.to_string(),
            value_type: "string".to_string(),
            requires_restart: false,
            is_sensitive: false,
            source: "system".to_string(),
            namespace: String::new(),
            category: "test".to_string(),
            description: "test".to_string(),
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    #[test]
    fn doctor_public_site_url_warns_for_http_origins() {
        let runtime_config = RuntimeConfig::new();
        runtime_config.apply(config_model(
            PUBLIC_SITE_URL_KEY,
            "http://drive.example.com",
        ));

        let check = doctor_public_site_url_check(&runtime_config);

        assert_eq!(check.status, DoctorStatus::Warn);
        assert_eq!(check.summary, "public_site_url uses insecure HTTP");
        assert!(
            check
                .details
                .iter()
                .any(|detail| { detail == "configured=http://drive.example.com" })
        );
        assert!(
            check
                .suggestion
                .as_deref()
                .is_some_and(|hint| hint.contains("https://"))
        );
    }

    #[test]
    fn doctor_public_site_url_accepts_https_origins() {
        let runtime_config = RuntimeConfig::new();
        runtime_config.apply(config_model(
            PUBLIC_SITE_URL_KEY,
            "https://drive.example.com",
        ));

        let check = doctor_public_site_url_check(&runtime_config);

        assert_eq!(check.status, DoctorStatus::Ok);
        assert_eq!(check.summary, "configured as https://drive.example.com");
    }
}
