use std::collections::HashMap;

use crate::db::repository::search_query::{
    escape_like_query, lower_like_condition, mysql_boolean_mode_query, sqlite_fts_match_condition,
    sqlite_match_query,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DbBackend, EntityTrait, ExprTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    sea_query::{Expr, Order, extension::postgres::PgExpr},
};

use crate::entities::{
    team,
    team_member::{self, Entity as TeamMember},
    user,
};
use crate::errors::{AsterError, Result};
use crate::types::{TeamMemberRole, UserStatus};

const SQLITE_USERS_FTS_TABLE: &str = "users_search_fts";

fn team_member_keyword_like_condition(keyword: &str) -> Condition {
    let mut condition = Condition::any()
        .add(lower_like_condition(
            (user::Entity, user::Column::Username),
            keyword,
        ))
        .add(lower_like_condition(
            (user::Entity, user::Column::Email),
            keyword,
        ));

    if let Ok(user_id) = keyword.parse::<i64>() {
        condition = condition.add(Expr::col((user::Entity, user::Column::Id)).eq(user_id));
    }

    condition
}

fn team_member_keyword_condition(backend: DbBackend, keyword: &str) -> Condition {
    let parsed_user_id = keyword.parse::<i64>().ok();

    match backend {
        DbBackend::Postgres => {
            let pattern = format!("%{}%", escape_like_query(keyword));
            let mut condition = Condition::any()
                .add(Expr::col((user::Entity, user::Column::Username)).ilike(pattern.clone()))
                .add(Expr::col((user::Entity, user::Column::Email)).ilike(pattern));

            if let Some(user_id) = parsed_user_id {
                condition = condition.add(user::Column::Id.eq(user_id));
            }

            condition
        }
        DbBackend::MySql => mysql_boolean_mode_query(keyword)
            .map(|boolean_query| {
                let mut condition = Condition::any().add(Expr::cust_with_exprs(
                    "MATCH(?, ?) AGAINST (? IN BOOLEAN MODE)",
                    [
                        Expr::col((user::Entity, user::Column::Username)),
                        Expr::col((user::Entity, user::Column::Email)),
                        Expr::val(boolean_query),
                    ],
                ));
                if let Some(user_id) = parsed_user_id {
                    condition =
                        condition.add(Expr::col((user::Entity, user::Column::Id)).eq(user_id));
                }
                condition
            })
            .unwrap_or_else(|| team_member_keyword_like_condition(keyword)),
        DbBackend::Sqlite => sqlite_match_query(keyword)
            .map(|match_query| {
                let mut condition = Condition::any().add(sqlite_fts_match_condition(
                    (user::Entity, user::Column::Id),
                    SQLITE_USERS_FTS_TABLE,
                    &match_query,
                ));
                if let Some(user_id) = parsed_user_id {
                    condition =
                        condition.add(Expr::col((user::Entity, user::Column::Id)).eq(user_id));
                }
                condition
            })
            .unwrap_or_else(|| team_member_keyword_like_condition(keyword)),
        _ => team_member_keyword_like_condition(keyword),
    }
}

fn team_member_role_rank_expr() -> sea_orm::sea_query::SimpleExpr {
    Expr::case(
        Expr::col((team_member::Entity, team_member::Column::Role)).eq(TeamMemberRole::Owner),
        0i32,
    )
    .case(
        Expr::col((team_member::Entity, team_member::Column::Role)).eq(TeamMemberRole::Admin),
        1i32,
    )
    .finally(2i32)
    .into()
}

pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: team_member::ActiveModel,
) -> Result<team_member::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn update<C: ConnectionTrait>(
    db: &C,
    model: team_member::ActiveModel,
) -> Result<team_member::Model> {
    model.update(db).await.map_err(AsterError::from)
}

pub async fn delete<C: ConnectionTrait>(db: &C, id: i64) -> Result<()> {
    let result = TeamMember::delete_by_id(id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    if result.rows_affected == 0 {
        return Err(AsterError::record_not_found(format!("team_member #{id}")));
    }
    Ok(())
}

pub async fn find_by_team_and_user<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    user_id: i64,
) -> Result<Option<team_member::Model>> {
    TeamMember::find()
        .filter(team_member::Column::TeamId.eq(team_id))
        .filter(team_member::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn list_by_user_with_team<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
) -> Result<Vec<(team_member::Model, team::Model)>> {
    list_by_user_with_team_for_archived_state(db, user_id, false).await
}

pub async fn list_by_user_with_archived_team<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
) -> Result<Vec<(team_member::Model, team::Model)>> {
    list_by_user_with_team_for_archived_state(db, user_id, true).await
}

async fn list_by_user_with_team_for_archived_state<C: ConnectionTrait>(
    db: &C,
    user_id: i64,
    archived: bool,
) -> Result<Vec<(team_member::Model, team::Model)>> {
    let memberships = TeamMember::find()
        .filter(team_member::Column::UserId.eq(user_id))
        .order_by_desc(team_member::Column::UpdatedAt)
        .all(db)
        .await
        .map_err(AsterError::from)?;

    if memberships.is_empty() {
        return Ok(vec![]);
    }

    let team_ids: Vec<i64> = memberships
        .iter()
        .map(|membership| membership.team_id)
        .collect();
    let mut team_query =
        team::Entity::find().filter(team::Column::Id.is_in(team_ids.iter().copied()));
    team_query = if archived {
        team_query.filter(team::Column::ArchivedAt.is_not_null())
    } else {
        team_query.filter(team::Column::ArchivedAt.is_null())
    };
    let team_map: HashMap<i64, team::Model> = team_query
        .all(db)
        .await
        .map_err(AsterError::from)?
        .into_iter()
        .map(|team| (team.id, team))
        .collect();

    Ok(memberships
        .into_iter()
        .filter_map(|membership| {
            team_map
                .get(&membership.team_id)
                .cloned()
                .map(|team| (membership, team))
        })
        .collect())
}

pub async fn list_by_team_with_user<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
) -> Result<Vec<(team_member::Model, user::Model)>> {
    let memberships = TeamMember::find()
        .filter(team_member::Column::TeamId.eq(team_id))
        .order_by_asc(team_member::Column::CreatedAt)
        .all(db)
        .await
        .map_err(AsterError::from)?;

    if memberships.is_empty() {
        return Ok(vec![]);
    }

    let user_ids: Vec<i64> = memberships
        .iter()
        .map(|membership| membership.user_id)
        .collect();
    let user_map: HashMap<i64, user::Model> = user::Entity::find()
        .filter(user::Column::Id.is_in(user_ids.iter().copied()))
        .all(db)
        .await
        .map_err(AsterError::from)?
        .into_iter()
        .map(|user| (user.id, user))
        .collect();

    Ok(memberships
        .into_iter()
        .filter_map(|membership| {
            user_map
                .get(&membership.user_id)
                .cloned()
                .map(|user| (membership, user))
        })
        .collect())
}

pub async fn list_page_by_team_with_user<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    role: Option<TeamMemberRole>,
    status: Option<UserStatus>,
    keyword: Option<&str>,
    limit: u64,
    offset: u64,
) -> Result<(Vec<(team_member::Model, user::Model)>, u64)> {
    let backend = db.get_database_backend();
    let mut query = TeamMember::find()
        .inner_join(user::Entity)
        .select_also(user::Entity)
        .filter(team_member::Column::TeamId.eq(team_id));

    if let Some(role) = role {
        query = query.filter(team_member::Column::Role.eq(role));
    }
    if let Some(status) = status {
        query = query.filter(user::Column::Status.eq(status));
    }
    if let Some(keyword) = keyword.map(str::trim).filter(|keyword| !keyword.is_empty()) {
        query = query.filter(team_member_keyword_condition(backend, keyword));
    }

    query = query
        .order_by(team_member_role_rank_expr(), Order::Asc)
        .order_by_asc(user::Column::Username)
        .order_by_asc(user::Column::Id);

    let total = query.clone().count(db).await.map_err(AsterError::from)?;
    if total == 0 || limit == 0 {
        return Ok((vec![], total));
    }

    let items = query
        .offset(offset)
        .limit(limit)
        .all(db)
        .await
        .map_err(AsterError::from)?
        .into_iter()
        .filter_map(|(membership, user)| user.map(|user| (membership, user)))
        .collect();

    Ok((items, total))
}

pub async fn count_by_team<C: ConnectionTrait>(db: &C, team_id: i64) -> Result<u64> {
    // Keep member counts aligned with list_by_team_with_user by only counting rows
    // that still join to a user record.
    let count = TeamMember::find()
        .select_only()
        .column_as(
            Expr::col((team_member::Entity, team_member::Column::Id)).count(),
            "member_count",
        )
        .inner_join(user::Entity)
        .filter(team_member::Column::TeamId.eq(team_id))
        .into_tuple::<i64>()
        .one(db)
        .await
        .map_err(AsterError::from)?
        .unwrap_or(0);

    Ok(count as u64)
}

pub async fn count_by_team_ids<C: ConnectionTrait>(
    db: &C,
    team_ids: &[i64],
) -> Result<HashMap<i64, u64>> {
    if team_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let counts = TeamMember::find()
        .select_only()
        .column(team_member::Column::TeamId)
        .column_as(
            Expr::col((team_member::Entity, team_member::Column::Id)).count(),
            "member_count",
        )
        .inner_join(user::Entity)
        .filter(team_member::Column::TeamId.is_in(team_ids.iter().copied()))
        .group_by(team_member::Column::TeamId)
        .into_tuple::<(i64, i64)>()
        .all(db)
        .await
        .map_err(AsterError::from)?;

    Ok(counts
        .into_iter()
        .map(|(team_id, member_count)| (team_id, member_count as u64))
        .collect())
}

pub async fn count_by_team_and_role<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
    role: TeamMemberRole,
) -> Result<u64> {
    let count = TeamMember::find()
        .select_only()
        .column_as(
            Expr::col((team_member::Entity, team_member::Column::Id)).count(),
            "member_count",
        )
        .inner_join(user::Entity)
        .filter(team_member::Column::TeamId.eq(team_id))
        .filter(team_member::Column::Role.eq(role))
        .into_tuple::<i64>()
        .one(db)
        .await
        .map_err(AsterError::from)?
        .unwrap_or(0);

    Ok(count as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DbBackend, QueryTrait};

    #[test]
    fn postgres_team_member_keyword_condition_uses_ilike_and_id_match() {
        let sql: String = format!(
            "{}",
            TeamMember::find()
                .inner_join(user::Entity)
                .filter(team_member_keyword_condition(DbBackend::Postgres, "alice"))
                .build(DbBackend::Postgres)
        );
        assert!(
            sql.as_str()
                .contains(r#""users"."username" ILIKE '%alice%'"#),
            "{sql}"
        );
        assert!(
            sql.as_str().contains(r#""users"."email" ILIKE '%alice%'"#),
            "{sql}"
        );

        let numeric_sql: String = format!(
            "{}",
            TeamMember::find()
                .inner_join(user::Entity)
                .filter(team_member_keyword_condition(DbBackend::Postgres, "42"))
                .build(DbBackend::Postgres)
        );
        assert!(
            numeric_sql.as_str().contains(r#""users"."id" = 42"#),
            "{numeric_sql}"
        );
    }

    #[test]
    fn mysql_team_member_keyword_condition_uses_match_against_and_id_match() {
        let sql: String = format!(
            "{}",
            TeamMember::find()
                .inner_join(user::Entity)
                .filter(team_member_keyword_condition(DbBackend::MySql, "alice"))
                .build(DbBackend::MySql)
        );
        assert!(sql.as_str().contains("MATCH("), "{sql}");
        assert!(sql.as_str().contains("`users`.`username`"), "{sql}");
        assert!(sql.as_str().contains("`users`.`email`"), "{sql}");
        assert!(
            sql.as_str()
                .contains(r#"AGAINST ('\"alice\"' IN BOOLEAN MODE)"#),
            "{sql}"
        );

        let numeric_sql: String = format!(
            "{}",
            TeamMember::find()
                .inner_join(user::Entity)
                .filter(team_member_keyword_condition(DbBackend::MySql, "42"))
                .build(DbBackend::MySql)
        );
        assert!(numeric_sql.as_str().contains("MATCH("), "{numeric_sql}");
        assert!(
            numeric_sql.as_str().contains("`users`.`id` = 42"),
            "{numeric_sql}"
        );
    }

    #[test]
    fn mysql_team_member_keyword_condition_falls_back_to_like_for_punctuation() {
        let sql: String = format!(
            "{}",
            TeamMember::find()
                .inner_join(user::Entity)
                .filter(team_member_keyword_condition(DbBackend::MySql, "end-u"))
                .build(DbBackend::MySql)
        );

        assert!(!sql.as_str().contains("MATCH("), "{sql}");
        assert!(
            sql.as_str()
                .contains("LOWER(`users`.`username`) LIKE '%end-u%'"),
            "{sql}"
        );
        assert!(
            sql.as_str()
                .contains("LOWER(`users`.`email`) LIKE '%end-u%'"),
            "{sql}"
        );
    }

    #[test]
    fn sqlite_team_member_keyword_condition_qualifies_user_id_for_join_queries() {
        let sql: String = format!(
            "{}",
            TeamMember::find()
                .inner_join(user::Entity)
                .filter(team_member_keyword_condition(DbBackend::Sqlite, "naly"))
                .build(DbBackend::Sqlite)
        );

        assert!(
            sql.as_str()
                .contains(r#""users"."id" IN (SELECT "rowid" FROM "users_search_fts""#),
            "{sql}"
        );
    }
}
