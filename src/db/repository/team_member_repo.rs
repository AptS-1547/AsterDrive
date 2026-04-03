use std::collections::HashMap;

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, ExprTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, sea_query::Expr,
};

use crate::entities::{
    team,
    team_member::{self, Entity as TeamMember},
    user,
};
use crate::errors::{AsterError, Result};
use crate::types::TeamMemberRole;

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
    let rows = TeamMember::find()
        .inner_join(team::Entity)
        .select_also(team::Entity)
        .filter(team_member::Column::UserId.eq(user_id))
        .filter(team::Column::ArchivedAt.is_null())
        .order_by_desc(team_member::Column::UpdatedAt)
        .all(db)
        .await
        .map_err(AsterError::from)?;

    Ok(rows
        .into_iter()
        .map(|(membership, maybe_team)| {
            let team = maybe_team.expect("inner_join(team) must load a team row");
            (membership, team)
        })
        .collect())
}

pub async fn list_by_team_with_user<C: ConnectionTrait>(
    db: &C,
    team_id: i64,
) -> Result<Vec<(team_member::Model, user::Model)>> {
    let rows = TeamMember::find()
        .inner_join(user::Entity)
        .select_also(user::Entity)
        .filter(team_member::Column::TeamId.eq(team_id))
        .order_by_asc(team_member::Column::CreatedAt)
        .all(db)
        .await
        .map_err(AsterError::from)?;

    Ok(rows
        .into_iter()
        .map(|(membership, maybe_user)| {
            let user = maybe_user.expect("inner_join(user) must load a user row");
            (membership, user)
        })
        .collect())
}

pub async fn count_by_team<C: ConnectionTrait>(db: &C, team_id: i64) -> Result<u64> {
    // Keep member counts aligned with list_by_team_with_user by only counting rows
    // that still join to a user record.
    TeamMember::find()
        .inner_join(user::Entity)
        .filter(team_member::Column::TeamId.eq(team_id))
        .count(db)
        .await
        .map_err(AsterError::from)
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
        .column_as(Expr::col(team_member::Column::Id).count(), "member_count")
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
    TeamMember::find()
        .filter(team_member::Column::TeamId.eq(team_id))
        .filter(team_member::Column::Role.eq(role))
        .count(db)
        .await
        .map_err(AsterError::from)
}
