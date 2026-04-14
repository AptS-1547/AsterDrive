use chrono::{DateTime, Duration, LocalResult, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use sea_orm::{
    ColumnTrait, DbBackend, EntityTrait, ExprTrait, PaginatorTrait, QueryFilter, QuerySelect,
    sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(all(debug_assertions, feature = "openapi"))]
use utoipa::{IntoParams, ToSchema};

use crate::entities::{
    audit_log::{self, Entity as AuditLog},
    file::{self, Entity as File},
    file_blob::Entity as FileBlob,
    share::Entity as Share,
    user::{self, Entity as User},
};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::AppState;
use crate::services::audit_service;
use crate::types::UserStatus;

type DateTimeUtc = DateTime<Utc>;

const DEFAULT_DAYS: u32 = 7;
const MAX_DAYS: u32 = 90;
const DEFAULT_EVENT_LIMIT: u64 = 8;
const MAX_EVENT_LIMIT: u64 = 50;
const DEFAULT_TIMEZONE: &str = "UTC";

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(IntoParams))]
pub struct AdminOverviewQuery {
    pub days: Option<u32>,
    pub timezone: Option<String>,
    pub event_limit: Option<u64>,
}

impl AdminOverviewQuery {
    pub fn days_or_default(&self) -> u32 {
        self.days
            .map(|days| days.clamp(1, MAX_DAYS))
            .unwrap_or(DEFAULT_DAYS)
    }

    pub fn event_limit_or_default(&self) -> u64 {
        self.event_limit
            .map(|limit| limit.clamp(1, MAX_EVENT_LIMIT))
            .unwrap_or(DEFAULT_EVENT_LIMIT)
    }

    pub fn timezone_name(&self) -> &str {
        self.timezone
            .as_deref()
            .filter(|timezone| !timezone.trim().is_empty())
            .unwrap_or(DEFAULT_TIMEZONE)
    }
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AdminOverviewStats {
    pub total_users: u64,
    pub active_users: u64,
    pub disabled_users: u64,
    pub total_files: u64,
    pub total_file_bytes: i64,
    pub total_blobs: u64,
    pub total_blob_bytes: i64,
    pub total_shares: u64,
    pub audit_events_today: u64,
    pub new_users_today: u64,
    pub uploads_today: u64,
    pub shares_today: u64,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AdminOverviewDailyReport {
    pub date: String,
    pub sign_ins: u64,
    pub new_users: u64,
    pub uploads: u64,
    pub share_creations: u64,
    pub deletions: u64,
    pub total_events: u64,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(all(debug_assertions, feature = "openapi"), derive(ToSchema))]
pub struct AdminOverview {
    #[cfg_attr(all(debug_assertions, feature = "openapi"), schema(value_type = String))]
    pub generated_at: DateTimeUtc,
    pub timezone: String,
    pub days: u32,
    pub stats: AdminOverviewStats,
    pub daily_reports: Vec<AdminOverviewDailyReport>,
    pub recent_events: Vec<audit_service::AuditLogEntry>,
}

pub async fn get_overview(
    state: &AppState,
    days: u32,
    timezone_name: &str,
    event_limit: u64,
) -> Result<AdminOverview> {
    let generated_at = Utc::now();
    let timezone = parse_timezone(timezone_name)?;
    let today = generated_at.with_timezone(&timezone).date_naive();

    let total_users = User::find().count(&state.db).await?;
    let active_users = User::find()
        .filter(user::Column::Status.eq(UserStatus::Active))
        .count(&state.db)
        .await?;
    let disabled_users = User::find()
        .filter(user::Column::Status.eq(UserStatus::Disabled))
        .count(&state.db)
        .await?;

    let total_files = File::find()
        .filter(file::Column::DeletedAt.is_null())
        .count(&state.db)
        .await?;
    let total_file_bytes = sum_live_file_bytes(state).await?;
    let total_blobs = FileBlob::find().count(&state.db).await?;
    let total_blob_bytes = sum_blob_bytes(state).await?;
    let total_shares = Share::find().count(&state.db).await?;

    let daily_reports = build_daily_reports(state, today, days, timezone).await?;
    let today_report = daily_reports
        .first()
        .cloned()
        .unwrap_or(AdminOverviewDailyReport {
            date: today.to_string(),
            sign_ins: 0,
            new_users: 0,
            uploads: 0,
            share_creations: 0,
            deletions: 0,
            total_events: 0,
        });
    let recent_events = audit_service::query(
        state,
        audit_service::AuditLogFilters {
            user_id: None,
            action: None,
            entity_type: None,
            entity_id: None,
            after: None,
            before: None,
        },
        event_limit,
        0,
    )
    .await?
    .items;

    Ok(AdminOverview {
        generated_at,
        timezone: timezone.name().to_string(),
        days,
        stats: AdminOverviewStats {
            total_users,
            active_users,
            disabled_users,
            total_files,
            total_file_bytes,
            total_blobs,
            total_blob_bytes,
            total_shares,
            audit_events_today: today_report.total_events,
            new_users_today: today_report.new_users,
            uploads_today: today_report.uploads,
            shares_today: today_report.share_creations,
        },
        daily_reports,
        recent_events,
    })
}

async fn build_daily_reports(
    state: &AppState,
    today: NaiveDate,
    days: u32,
    timezone: Tz,
) -> Result<Vec<AdminOverviewDailyReport>> {
    let mut reports = Vec::with_capacity(days as usize);
    let mut report_indexes = HashMap::with_capacity(days as usize);

    for offset in 0..days {
        let date = today - Duration::days(offset as i64);
        report_indexes.insert(date, reports.len());

        reports.push(AdminOverviewDailyReport {
            date: date.to_string(),
            sign_ins: 0,
            new_users: 0,
            uploads: 0,
            share_creations: 0,
            deletions: 0,
            total_events: 0,
        });
    }

    let oldest_date = today - Duration::days(days.saturating_sub(1) as i64);
    let start = start_of_local_day(oldest_date, timezone)?;
    let end = start_of_local_day(today + Duration::days(1), timezone)?;

    let events = AuditLog::find()
        .select_only()
        .column(audit_log::Column::Action)
        .column(audit_log::Column::CreatedAt)
        .filter(audit_log::Column::CreatedAt.gte(start))
        .filter(audit_log::Column::CreatedAt.lt(end))
        .into_tuple::<(String, DateTimeUtc)>()
        .all(&state.db)
        .await?;

    for (action, created_at) in events {
        let date = created_at.with_timezone(&timezone).date_naive();
        let Some(report_index) = report_indexes.get(&date).copied() else {
            continue;
        };
        let report = &mut reports[report_index];
        record_audit_action(report, action.as_str());
    }

    Ok(reports)
}

fn record_audit_action(report: &mut AdminOverviewDailyReport, action: &str) {
    report.total_events += 1;

    match action {
        "user_login" => report.sign_ins += 1,
        "user_register" | "admin_create_user" => report.new_users += 1,
        "file_upload" => report.uploads += 1,
        "share_create" => report.share_creations += 1,
        "batch_delete" | "file_delete" | "folder_delete" => report.deletions += 1,
        _ => {}
    }
}

async fn sum_live_file_bytes(state: &AppState) -> Result<i64> {
    Ok(File::find()
        .select_only()
        .column_as(
            sum_as_i64_expr(state.db.get_database_backend(), file::Column::Size),
            "sum",
        )
        .filter(file::Column::DeletedAt.is_null())
        .into_tuple::<Option<i64>>()
        .one(&state.db)
        .await?
        .flatten()
        .unwrap_or(0))
}

async fn sum_blob_bytes(state: &AppState) -> Result<i64> {
    Ok(FileBlob::find()
        .select_only()
        .column_as(
            sum_as_i64_expr(
                state.db.get_database_backend(),
                crate::entities::file_blob::Column::Size,
            ),
            "sum",
        )
        .into_tuple::<Option<i64>>()
        .one(&state.db)
        .await?
        .flatten()
        .unwrap_or(0))
}

fn sum_as_i64_expr(
    backend: DbBackend,
    column: impl sea_orm::sea_query::IntoColumnRef + Copy,
) -> sea_orm::sea_query::SimpleExpr {
    let type_name = match backend {
        DbBackend::Postgres => "bigint",
        DbBackend::MySql => "signed",
        _ => "integer",
    };

    Expr::col(column).sum().cast_as(type_name)
}

fn parse_timezone(timezone_name: &str) -> Result<Tz> {
    timezone_name.parse::<Tz>().map_aster_err_with(|| {
        AsterError::validation_error(format!("invalid timezone '{timezone_name}'"))
    })
}

fn start_of_local_day(date: NaiveDate, timezone: Tz) -> Result<DateTimeUtc> {
    let naive = date
        .and_hms_opt(0, 0, 0)
        .expect("start of day should always be valid");
    match timezone.from_local_datetime(&naive) {
        LocalResult::Single(dt) => Ok(dt.with_timezone(&Utc)),
        LocalResult::Ambiguous(earliest, _) => Ok(earliest.with_timezone(&Utc)),
        LocalResult::None => Err(AsterError::validation_error(format!(
            "timezone '{}' cannot represent local midnight for {}",
            timezone.name(),
            date
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{AdminOverviewDailyReport, record_audit_action, sum_as_i64_expr};
    use sea_orm::{
        DbBackend,
        sea_query::{PostgresQueryBuilder, Query},
    };

    fn empty_report() -> AdminOverviewDailyReport {
        AdminOverviewDailyReport {
            date: "2026-03-30".to_string(),
            sign_ins: 0,
            new_users: 0,
            uploads: 0,
            share_creations: 0,
            deletions: 0,
            total_events: 0,
        }
    }

    #[test]
    fn record_audit_action_counts_categories() {
        let mut report = empty_report();

        for action in [
            "user_login",
            "user_register",
            "admin_create_user",
            "file_upload",
            "share_create",
            "batch_delete",
            "file_delete",
            "folder_delete",
            "ignored",
        ] {
            record_audit_action(&mut report, action);
        }

        assert_eq!(report.sign_ins, 1);
        assert_eq!(report.new_users, 2);
        assert_eq!(report.uploads, 1);
        assert_eq!(report.share_creations, 1);
        assert_eq!(report.deletions, 3);
        assert_eq!(report.total_events, 9);
    }

    #[test]
    fn postgres_sum_expr_casts_to_bigint() {
        let sql = Query::select()
            .expr(sum_as_i64_expr(
                DbBackend::Postgres,
                super::file::Column::Size,
            ))
            .from(super::File)
            .to_string(PostgresQueryBuilder);

        assert!(sql.contains(r#"CAST(SUM("size") AS bigint)"#), "{sql}");
    }
}
