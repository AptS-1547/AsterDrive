use std::collections::HashSet;
use std::path::Path;

use crate::errors::{AsterError, Result};
use clap::Args;
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
    status: DoctorStatus,
    database_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    backend: Option<String>,
    summary: DoctorSummary,
    checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    fn new(
        database_url: String,
        backend: Option<String>,
        strict: bool,
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

        let status = if fail > 0 || (strict && warn > 0) {
            DoctorStatus::Fail
        } else if warn > 0 {
            DoctorStatus::Warn
        } else {
            DoctorStatus::Ok
        };

        Self {
            strict,
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

pub async fn execute_doctor_command(args: &DoctorArgs) -> DoctorReport {
    let redacted_database_url = redact_database_url(&args.database_url);
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
                    "检查 --database-url、数据库服务状态，以及目标库访问权限".to_string(),
                ),
            });
            None
        }
    };

    let Some((db, db_backend)) = db else {
        return DoctorReport::new(redacted_database_url, backend, args.strict, checks);
    };

    checks.push(match doctor_pending_migrations(&db, db_backend).await {
        Ok(pending) if pending.is_empty() => DoctorCheck {
            name: "database_migrations",
            label: "Database migrations",
            status: DoctorStatus::Ok,
            summary: "no pending migrations".to_string(),
            details: Vec::new(),
            suggestion: None,
        },
        Ok(pending) => DoctorCheck {
            name: "database_migrations",
            label: "Database migrations",
            status: DoctorStatus::Warn,
            summary: format!("{} pending migration(s)", pending.len()),
            details: pending,
            suggestion: Some("先补齐数据库迁移，再执行维护类 CLI 操作".to_string()),
        },
        Err(err) => DoctorCheck {
            name: "database_migrations",
            label: "Database migrations",
            status: DoctorStatus::Fail,
            summary: "failed to inspect migration history".to_string(),
            details: vec![err.message().to_string()],
            suggestion: Some(
                "检查 seaql_migrations 表和数据库权限，确认迁移元数据可读".to_string(),
            ),
        },
    });

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
                suggestion: Some("检查 system_config 表结构和配置数据是否完整".to_string()),
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

    DoctorReport::new(redacted_database_url, backend, args.strict, checks)
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
            if report.strict {
                "strict (warnings fail)"
            } else {
                "standard"
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
                "设置 config public_site_url 为用户可访问的外部 HTTP(S) 域名".to_string(),
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
                "设置 config public_site_url 为用户可访问的外部 HTTP(S) 域名".to_string(),
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
                        "把站点放到 HTTPS 反向代理后面，再把 public_site_url 改成 https:// 域名"
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
                "只保留纯 origin，例如 https://drive.example.com，不要带路径或非 HTTP(S) 协议"
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
                "要么同时设置 mail_smtp_username / mail_smtp_password，要么两者都留空".to_string(),
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
                "至少补齐 mail_smtp_host 和 mail_from_address，发信功能才算可用".to_string(),
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
                        "修正 frontend_preview_apps_json，或者先恢复为系统默认预览配置".to_string(),
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
                        "检查 frontend_preview_apps_json 是否被手工编辑坏了，必要时导回默认值"
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
                "补上 public_site_url，或者先禁用 WOPI 预览应用，避免生成不可用的预览入口"
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
                    "确认数据库已迁移完成，并且 storage_policies 表可正常访问".to_string(),
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
                    "确认数据库已迁移完成，并且 storage_policy_groups 表可正常访问".to_string(),
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
            suggestion: Some("检查策略、策略组和用户策略组分配数据是否一致".to_string()),
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
                "先启动一次服务端，或手工补齐默认 storage policy / policy group 的种子数据"
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
        .map_err(|error| AsterError::database_operation(error.to_string()))?;

    rows.into_iter()
        .map(|row| {
            row.try_get_by_index::<String>(0)
                .map_err(|error| AsterError::database_operation(error.to_string()))
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
        .map_err(|error| AsterError::database_operation(error.to_string()))?
        .ok_or_else(|| AsterError::database_operation("table existence query returned no rows"))?;
    let exists = row
        .try_get_by_index::<i64>(0)
        .map_err(|error| AsterError::database_operation(error.to_string()))?;
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
