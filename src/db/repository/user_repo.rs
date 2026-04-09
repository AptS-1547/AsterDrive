use crate::db::repository::pagination_repo::fetch_offset_page;
use crate::entities::user::{self, Entity as User};
use crate::errors::{AsterError, Result};
use crate::types::{UserRole, UserStatus};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, PaginatorTrait,
    QueryFilter, QueryOrder, sea_query::Expr,
};

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<user::Model> {
    User::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("user #{id}")))
}

pub async fn find_by_ids<C: ConnectionTrait>(db: &C, ids: &[i64]) -> Result<Vec<user::Model>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }

    User::find()
        .filter(user::Column::Id.is_in(ids.iter().copied()))
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_username<C: ConnectionTrait>(
    db: &C,
    username: &str,
) -> Result<Option<user::Model>> {
    User::find()
        .filter(user::Column::Username.eq(username))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_email<C: ConnectionTrait>(db: &C, email: &str) -> Result<Option<user::Model>> {
    User::find()
        .filter(user::Column::Email.eq(email))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_pending_email<C: ConnectionTrait>(
    db: &C,
    email: &str,
) -> Result<Option<user::Model>> {
    User::find()
        .filter(user::Column::PendingEmail.eq(email))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<user::Model>> {
    User::find()
        .order_by_asc(user::Column::Id)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_by_policy_group<C: ConnectionTrait>(
    db: &C,
    policy_group_id: i64,
) -> Result<Vec<user::Model>> {
    User::find()
        .filter(user::Column::PolicyGroupId.eq(policy_group_id))
        .order_by_asc(user::Column::Id)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn count_by_policy_group<C: ConnectionTrait>(
    db: &C,
    policy_group_id: i64,
) -> Result<u64> {
    User::find()
        .filter(user::Column::PolicyGroupId.eq(policy_group_id))
        .count(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_paginated<C: ConnectionTrait>(
    db: &C,
    limit: u64,
    offset: u64,
    keyword: Option<&str>,
    role: Option<UserRole>,
    status: Option<UserStatus>,
) -> Result<(Vec<user::Model>, u64)> {
    let mut q = User::find().order_by_asc(user::Column::Id);

    if let Some(keyword) = keyword.filter(|s| !s.trim().is_empty()) {
        let pattern = format!("%{}%", keyword.trim());
        q = q.filter(
            sea_orm::Condition::any()
                .add(user::Column::Username.like(&pattern))
                .add(user::Column::Email.like(&pattern)),
        );
    }
    if let Some(role) = role {
        q = q.filter(user::Column::Role.eq(role));
    }
    if let Some(status) = status {
        q = q.filter(user::Column::Status.eq(status));
    }

    fetch_offset_page(db, q, limit, offset).await
}

pub async fn count_all<C: ConnectionTrait>(db: &C) -> Result<u64> {
    User::find().count(db).await.map_err(AsterError::from)
}

pub async fn create<C: ConnectionTrait>(db: &C, model: user::ActiveModel) -> Result<user::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

/// 检查用户配额是否足够。quota=0 表示不限。
pub async fn check_quota<C: ConnectionTrait>(db: &C, user_id: i64, needed_size: i64) -> Result<()> {
    let user = find_by_id(db, user_id).await?;
    if user.storage_quota > 0 && user.storage_used + needed_size > user.storage_quota {
        return Err(AsterError::storage_quota_exceeded(format!(
            "quota {}, used {}, need {}",
            user.storage_quota, user.storage_used, needed_size
        )));
    }
    Ok(())
}

pub async fn update_storage_used<C: ConnectionTrait>(db: &C, id: i64, delta: i64) -> Result<()> {
    let expr = if delta >= 0 {
        Expr::col(user::Column::StorageUsed).add(delta)
    } else {
        let decrement_by = -delta;
        Expr::case(Expr::col(user::Column::StorageUsed).lt(decrement_by), 0)
            .finally(Expr::col(user::Column::StorageUsed).sub(decrement_by))
            .into()
    };

    let result = User::update_many()
        .col_expr(user::Column::StorageUsed, expr)
        .filter(user::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;

    if result.rows_affected == 0 {
        return Err(AsterError::record_not_found(format!("user #{id}")));
    }

    Ok(())
}

pub async fn bump_session_version<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    let result = User::update_many()
        .col_expr(
            user::Column::SessionVersion,
            Expr::col(user::Column::SessionVersion).add(1i64),
        )
        .filter(user::Column::Id.eq(id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;

    if result.rows_affected == 0 {
        return Err(AsterError::record_not_found(format!("user #{id}")));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, QueryTrait};

    #[test]
    fn postgres_update_storage_used_sql_is_valid() {
        let sql = User::update_many()
            .col_expr(
                user::Column::StorageUsed,
                Expr::col(user::Column::StorageUsed).add(1i64),
            )
            .filter(user::Column::Id.eq(7))
            .build(DbBackend::Postgres)
            .to_string();

        assert!(
            sql.contains(r#""storage_used" = "storage_used" + 1"#),
            "{sql}"
        );
        assert!(sql.contains(r#"WHERE "users"."id" = 7"#), "{sql}");
    }

    #[test]
    fn postgres_bump_session_version_sql_is_valid() {
        let sql = User::update_many()
            .col_expr(
                user::Column::SessionVersion,
                Expr::col(user::Column::SessionVersion).add(1i64),
            )
            .filter(user::Column::Id.eq(9))
            .build(DbBackend::Postgres)
            .to_string();

        assert!(
            sql.contains(r#""session_version" = "session_version" + 1"#),
            "{sql}"
        );
        assert!(sql.contains(r#"WHERE "users"."id" = 9"#), "{sql}");
    }
}
