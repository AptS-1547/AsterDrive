use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveEnum, ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait, ExprTrait,
    QueryFilter, QueryOrder, QuerySelect, sea_query::Expr,
};

use crate::db::repository::pagination_repo::fetch_offset_page;
use crate::entities::background_task::{self, Entity as BackgroundTask};
use crate::errors::{AsterError, Result};
use crate::types::BackgroundTaskStatus;

pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: background_task::ActiveModel,
) -> Result<background_task::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<background_task::Model> {
    BackgroundTask::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("task #{id}")))
}

pub async fn find_paginated_personal<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    limit: u64,
    offset: u64,
) -> Result<(Vec<background_task::Model>, u64)> {
    fetch_offset_page(
        db,
        BackgroundTask::find()
            .filter(background_task::Column::CreatorUserId.eq(user_id))
            .filter(background_task::Column::TeamId.is_null())
            .order_by_desc(background_task::Column::CreatedAt),
        limit,
        offset,
    )
    .await
}

pub async fn find_paginated_team<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    limit: u64,
    offset: u64,
) -> Result<(Vec<background_task::Model>, u64)> {
    fetch_offset_page(
        db,
        BackgroundTask::find()
            .filter(background_task::Column::TeamId.eq(team_id))
            .order_by_desc(background_task::Column::CreatedAt),
        limit,
        offset,
    )
    .await
}

pub async fn find_paginated_all<C: ConnectionTrait>(
    db: &C,
    limit: u64,
    offset: u64,
) -> Result<(Vec<background_task::Model>, u64)> {
    fetch_offset_page(
        db,
        BackgroundTask::find().order_by_desc(background_task::Column::UpdatedAt),
        limit,
        offset,
    )
    .await
}

pub async fn list_recent<C: ConnectionTrait>(
    db: &C,
    limit: u64,
) -> Result<Vec<background_task::Model>> {
    BackgroundTask::find()
        .order_by_desc(background_task::Column::UpdatedAt)
        .limit(limit)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn list_claimable<C: ConnectionTrait>(
    db: &C,
    now: DateTime<Utc>,
    stale_before: DateTime<Utc>,
    limit: u64,
) -> Result<Vec<background_task::Model>> {
    BackgroundTask::find()
        .filter(claimable_condition(now, stale_before))
        .order_by_asc(background_task::Column::CreatedAt)
        .limit(limit)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn try_claim<C: ConnectionTrait>(
    db: &C,
    id: i64,
    expected_processing_token: i64,
    now: DateTime<Utc>,
    stale_before: DateTime<Utc>,
    next_processing_token: i64,
    lease_expires_at: DateTime<Utc>,
) -> Result<bool> {
    // try_claim 是一条 compare-and-swap：
    // 只有当 id 命中、旧 processing_token 仍匹配、并且任务此刻仍满足 claimable 条件时，
    // 才会把任务推进到 Processing，并原子地把 token 递增到 next_processing_token。
    //
    // 这样多个 dispatcher 并发捞到同一条任务时，只有一个能成功认领。
    let result = BackgroundTask::update_many()
        .col_expr(
            background_task::Column::Status,
            Expr::value(BackgroundTaskStatus::Processing.to_value()),
        )
        .col_expr(
            background_task::Column::ProcessingStartedAt,
            Expr::value(Some(now)),
        )
        .col_expr(
            background_task::Column::LastHeartbeatAt,
            Expr::value(Some(now)),
        )
        .col_expr(
            background_task::Column::ProcessingToken,
            Expr::value(next_processing_token),
        )
        .col_expr(
            background_task::Column::LeaseExpiresAt,
            Expr::value(Some(lease_expires_at)),
        )
        .col_expr(
            background_task::Column::StartedAt,
            Expr::col(background_task::Column::StartedAt).if_null(now),
        )
        .col_expr(background_task::Column::UpdatedAt, Expr::value(now))
        .filter(background_task::Column::Id.eq(id))
        .filter(background_task::Column::ProcessingToken.eq(expected_processing_token))
        .filter(claimable_condition(now, stale_before))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(result.rows_affected == 1)
}

pub async fn mark_progress<C: ConnectionTrait>(
    db: &C,
    id: i64,
    processing_token: i64,
    now: DateTime<Utc>,
    lease_expires_at: DateTime<Utc>,
    current: i64,
    total: i64,
    status_text: Option<&str>,
    steps_json: Option<&str>,
) -> Result<bool> {
    let mut update = BackgroundTask::update_many()
        .col_expr(
            background_task::Column::ProgressCurrent,
            Expr::value(current),
        )
        .col_expr(background_task::Column::ProgressTotal, Expr::value(total))
        .col_expr(
            background_task::Column::StatusText,
            Expr::value(status_text.map(str::to_string)),
        )
        .col_expr(
            background_task::Column::LastHeartbeatAt,
            Expr::value(Some(now)),
        )
        .col_expr(
            background_task::Column::LeaseExpiresAt,
            Expr::value(Some(lease_expires_at)),
        )
        .col_expr(background_task::Column::UpdatedAt, Expr::value(now))
        .filter(background_task::Column::Id.eq(id))
        .filter(background_task::Column::Status.eq(BackgroundTaskStatus::Processing))
        .filter(background_task::Column::ProcessingToken.eq(processing_token));
    if let Some(steps_json) = steps_json {
        update = update.col_expr(
            background_task::Column::StepsJson,
            Expr::value(Some(steps_json.to_string())),
        );
    }
    let result = update.exec(db).await.map_err(AsterError::from)?;
    Ok(result.rows_affected == 1)
}

#[allow(clippy::too_many_arguments)]
pub async fn mark_succeeded<C: ConnectionTrait>(
    db: &C,
    id: i64,
    processing_token: i64,
    result_json: Option<&str>,
    steps_json: Option<&str>,
    current: i64,
    total: i64,
    status_text: Option<&str>,
    finished_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
) -> Result<bool> {
    let mut update = BackgroundTask::update_many()
        .col_expr(
            background_task::Column::Status,
            Expr::value(BackgroundTaskStatus::Succeeded.to_value()),
        )
        .col_expr(
            background_task::Column::ResultJson,
            Expr::value(result_json.map(str::to_string)),
        )
        .col_expr(
            background_task::Column::ProgressCurrent,
            Expr::value(current),
        )
        .col_expr(background_task::Column::ProgressTotal, Expr::value(total))
        .col_expr(
            background_task::Column::StatusText,
            Expr::value(status_text.map(str::to_string)),
        )
        .col_expr(
            background_task::Column::ProcessingStartedAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::LastHeartbeatAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::LeaseExpiresAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::FinishedAt,
            Expr::value(Some(finished_at)),
        )
        .col_expr(background_task::Column::ExpiresAt, Expr::value(expires_at))
        .col_expr(background_task::Column::UpdatedAt, Expr::value(finished_at))
        .filter(background_task::Column::Id.eq(id))
        .filter(background_task::Column::Status.eq(BackgroundTaskStatus::Processing))
        .filter(background_task::Column::ProcessingToken.eq(processing_token));
    if let Some(steps_json) = steps_json {
        update = update.col_expr(
            background_task::Column::StepsJson,
            Expr::value(Some(steps_json.to_string())),
        );
    }
    let result = update.exec(db).await.map_err(AsterError::from)?;
    Ok(result.rows_affected == 1)
}

pub async fn mark_retry<C: ConnectionTrait>(
    db: &C,
    id: i64,
    processing_token: i64,
    attempt_count: i32,
    next_run_at: DateTime<Utc>,
    last_error: &str,
    steps_json: Option<&str>,
) -> Result<bool> {
    let mut update = BackgroundTask::update_many()
        .col_expr(
            background_task::Column::Status,
            Expr::value(BackgroundTaskStatus::Retry.to_value()),
        )
        .col_expr(
            background_task::Column::AttemptCount,
            Expr::value(attempt_count),
        )
        .col_expr(background_task::Column::NextRunAt, Expr::value(next_run_at))
        .col_expr(
            background_task::Column::ProcessingStartedAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::LastHeartbeatAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::LeaseExpiresAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::StatusText,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            background_task::Column::LastError,
            Expr::value(Some(last_error.to_string())),
        )
        .col_expr(background_task::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(background_task::Column::Id.eq(id))
        .filter(background_task::Column::Status.eq(BackgroundTaskStatus::Processing))
        .filter(background_task::Column::ProcessingToken.eq(processing_token));
    if let Some(steps_json) = steps_json {
        update = update.col_expr(
            background_task::Column::StepsJson,
            Expr::value(Some(steps_json.to_string())),
        );
    }
    let result = update.exec(db).await.map_err(AsterError::from)?;
    Ok(result.rows_affected == 1)
}

pub async fn mark_failed<C: ConnectionTrait>(
    db: &C,
    id: i64,
    processing_token: i64,
    attempt_count: i32,
    last_error: &str,
    finished_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    steps_json: Option<&str>,
) -> Result<bool> {
    let mut update = BackgroundTask::update_many()
        .col_expr(
            background_task::Column::Status,
            Expr::value(BackgroundTaskStatus::Failed.to_value()),
        )
        .col_expr(
            background_task::Column::AttemptCount,
            Expr::value(attempt_count),
        )
        .col_expr(
            background_task::Column::ProcessingStartedAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::LastHeartbeatAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::LeaseExpiresAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::StatusText,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            background_task::Column::LastError,
            Expr::value(Some(last_error.to_string())),
        )
        .col_expr(
            background_task::Column::FinishedAt,
            Expr::value(Some(finished_at)),
        )
        .col_expr(background_task::Column::ExpiresAt, Expr::value(expires_at))
        .col_expr(background_task::Column::UpdatedAt, Expr::value(finished_at))
        .filter(background_task::Column::Id.eq(id))
        .filter(background_task::Column::Status.eq(BackgroundTaskStatus::Processing))
        .filter(background_task::Column::ProcessingToken.eq(processing_token));
    if let Some(steps_json) = steps_json {
        update = update.col_expr(
            background_task::Column::StepsJson,
            Expr::value(Some(steps_json.to_string())),
        );
    }
    let result = update.exec(db).await.map_err(AsterError::from)?;
    Ok(result.rows_affected == 1)
}

pub async fn reset_for_manual_retry<C: ConnectionTrait>(
    db: &C,
    id: i64,
    now: DateTime<Utc>,
    max_attempts: i32,
    steps_json: Option<&str>,
) -> Result<bool> {
    let mut update = BackgroundTask::update_many()
        .col_expr(
            background_task::Column::Status,
            Expr::value(BackgroundTaskStatus::Pending.to_value()),
        )
        .col_expr(background_task::Column::AttemptCount, Expr::value(0))
        .col_expr(background_task::Column::ProgressCurrent, Expr::value(0))
        .col_expr(
            background_task::Column::MaxAttempts,
            Expr::value(max_attempts),
        )
        .col_expr(background_task::Column::NextRunAt, Expr::value(now))
        .col_expr(
            background_task::Column::ProcessingStartedAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::LastHeartbeatAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::LeaseExpiresAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::StartedAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::FinishedAt,
            Expr::value(Option::<DateTime<Utc>>::None),
        )
        .col_expr(
            background_task::Column::StatusText,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            background_task::Column::LastError,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            background_task::Column::ResultJson,
            Expr::value(Option::<String>::None),
        )
        .col_expr(background_task::Column::UpdatedAt, Expr::value(now))
        .filter(background_task::Column::Id.eq(id))
        .filter(background_task::Column::Status.eq(BackgroundTaskStatus::Failed));
    if let Some(steps_json) = steps_json {
        update = update.col_expr(
            background_task::Column::StepsJson,
            Expr::value(Some(steps_json.to_string())),
        );
    }
    let result = update.exec(db).await.map_err(AsterError::from)?;
    Ok(result.rows_affected == 1)
}

pub async fn touch_heartbeat<C: ConnectionTrait>(
    db: &C,
    id: i64,
    processing_token: i64,
    now: DateTime<Utc>,
    lease_expires_at: DateTime<Utc>,
) -> Result<bool> {
    // heartbeat 也带 token 条件。
    // 如果返回 false，说明任务虽然还在表里，但这条 worker 的 lease 已经过期了。
    let result = BackgroundTask::update_many()
        .col_expr(
            background_task::Column::LastHeartbeatAt,
            Expr::value(Some(now)),
        )
        .col_expr(
            background_task::Column::LeaseExpiresAt,
            Expr::value(Some(lease_expires_at)),
        )
        .filter(background_task::Column::Id.eq(id))
        .filter(background_task::Column::Status.eq(BackgroundTaskStatus::Processing))
        .filter(background_task::Column::ProcessingToken.eq(processing_token))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(result.rows_affected == 1)
}

pub async fn list_expired_terminal<C: ConnectionTrait>(
    db: &C,
    now: DateTime<Utc>,
    limit: u64,
) -> Result<Vec<background_task::Model>> {
    BackgroundTask::find()
        .filter(background_task::Column::ExpiresAt.lte(now))
        .filter(background_task::Column::Status.is_in([
            BackgroundTaskStatus::Succeeded,
            BackgroundTaskStatus::Failed,
            BackgroundTaskStatus::Canceled,
        ]))
        .order_by_asc(background_task::Column::ExpiresAt)
        .limit(limit)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn delete_many<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }
    Ok(BackgroundTask::delete_many()
        .filter(background_task::Column::Id.is_in(ids.iter().copied()))
        .exec(db)
        .await
        .map_err(AsterError::from)?
        .rows_affected)
}

fn claimable_condition(now: DateTime<Utc>, stale_before: DateTime<Utc>) -> Condition {
    // 可认领任务有两类：
    // 1. Pending / Retry 且 next_run_at 已到；
    // 2. 仍显示 Processing，但已经 stale，可被新 worker 硬接管。
    Condition::any()
        .add(
            Condition::all()
                .add(
                    background_task::Column::Status
                        .is_in([BackgroundTaskStatus::Pending, BackgroundTaskStatus::Retry]),
                )
                .add(background_task::Column::NextRunAt.lte(now)),
        )
        .add(processing_stale_condition(now, stale_before))
}

fn processing_stale_condition(now: DateTime<Utc>, stale_before: DateTime<Utc>) -> Condition {
    // 新记录优先使用显式 lease_expires_at 判定是否可接管；
    // 只有旧数据或迁移过渡期没有 lease_expires_at 时，才回退到 heartbeat/started_at 逻辑。
    Condition::any()
        .add(
            Condition::all()
                .add(background_task::Column::Status.eq(BackgroundTaskStatus::Processing))
                .add(background_task::Column::LeaseExpiresAt.is_not_null())
                .add(background_task::Column::LeaseExpiresAt.lte(now)),
        )
        .add(
            Condition::all()
                .add(background_task::Column::Status.eq(BackgroundTaskStatus::Processing))
                .add(background_task::Column::LeaseExpiresAt.is_null())
                .add(background_task::Column::LastHeartbeatAt.is_not_null())
                .add(background_task::Column::LastHeartbeatAt.lte(stale_before)),
        )
        .add(
            Condition::all()
                .add(background_task::Column::Status.eq(BackgroundTaskStatus::Processing))
                .add(background_task::Column::LeaseExpiresAt.is_null())
                .add(background_task::Column::LastHeartbeatAt.is_null())
                .add(background_task::Column::ProcessingStartedAt.lte(stale_before)),
        )
}
