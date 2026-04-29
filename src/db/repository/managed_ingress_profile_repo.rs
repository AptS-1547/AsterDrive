//! 仓储模块：`managed_ingress_profile_repo`。

use crate::entities::managed_ingress_profile::{self, Entity as ManagedIngressProfile};
use crate::errors::{AsterError, Result, validation_error_with_subcode};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set,
};

pub async fn find_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<managed_ingress_profile::Model> {
    ManagedIngressProfile::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("managed_ingress_profile #{id}")))
}

pub async fn find_by_binding_and_profile_key<C: ConnectionTrait>(
    db: &C,
    master_binding_id: i64,
    profile_key: &str,
) -> Result<Option<managed_ingress_profile::Model>> {
    ManagedIngressProfile::find()
        .filter(managed_ingress_profile::Column::MasterBindingId.eq(master_binding_id))
        .filter(managed_ingress_profile::Column::ProfileKey.eq(profile_key))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_all_by_binding<C: ConnectionTrait>(
    db: &C,
    master_binding_id: i64,
) -> Result<Vec<managed_ingress_profile::Model>> {
    ManagedIngressProfile::find()
        .filter(managed_ingress_profile::Column::MasterBindingId.eq(master_binding_id))
        .order_by_desc(managed_ingress_profile::Column::IsDefault)
        .order_by_desc(managed_ingress_profile::Column::CreatedAt)
        .order_by_desc(managed_ingress_profile::Column::Id)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_default_by_binding<C: ConnectionTrait>(
    db: &C,
    master_binding_id: i64,
) -> Result<Option<managed_ingress_profile::Model>> {
    ManagedIngressProfile::find()
        .filter(managed_ingress_profile::Column::MasterBindingId.eq(master_binding_id))
        .filter(managed_ingress_profile::Column::IsDefault.eq(true))
        .order_by_desc(managed_ingress_profile::Column::UpdatedAt)
        .order_by_desc(managed_ingress_profile::Column::Id)
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn count_by_binding<C: ConnectionTrait>(db: &C, master_binding_id: i64) -> Result<u64> {
    ManagedIngressProfile::find()
        .filter(managed_ingress_profile::Column::MasterBindingId.eq(master_binding_id))
        .count(db)
        .await
        .map_err(AsterError::from)
}

pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: managed_ingress_profile::ActiveModel,
) -> Result<managed_ingress_profile::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn update<C: ConnectionTrait>(
    db: &C,
    model: managed_ingress_profile::ActiveModel,
) -> Result<managed_ingress_profile::Model> {
    model.update(db).await.map_err(AsterError::from)
}

pub async fn delete_by_binding_and_profile_key<C: ConnectionTrait>(
    db: &C,
    master_binding_id: i64,
    profile_key: &str,
) -> Result<()> {
    let model = find_by_binding_and_profile_key(db, master_binding_id, profile_key)
        .await?
        .ok_or_else(|| {
            AsterError::record_not_found(format!("managed_ingress_profile '{profile_key}'"))
        })?;
    ManagedIngressProfile::delete_by_id(model.id)
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// Must be called inside a transaction, for example through `with_transaction`.
///
/// This loads and validates the target via `find_by_id`, clears existing
/// defaults with `update_many`, then marks the target with `update`; without
/// transaction semantics, concurrent readers can observe a temporary no-default
/// state and concurrent writers can create conflicting defaults.
pub async fn set_only_default_for_binding(
    db: &DatabaseTransaction,
    master_binding_id: i64,
    profile_id: i64,
) -> Result<()> {
    let existing = find_by_id(db, profile_id).await?;
    if existing.master_binding_id != master_binding_id {
        return Err(validation_error_with_subcode(
            "managed_ingress.binding_mismatch",
            format!(
                "managed ingress profile #{profile_id} does not belong to master_binding #{master_binding_id}"
            ),
        ));
    }

    ManagedIngressProfile::update_many()
        .filter(managed_ingress_profile::Column::MasterBindingId.eq(master_binding_id))
        .col_expr(
            managed_ingress_profile::Column::IsDefault,
            Expr::value(false),
        )
        .exec(db)
        .await
        .map_err(AsterError::from)?;

    let mut active: managed_ingress_profile::ActiveModel = existing.into();
    active.is_default = Set(true);
    active.updated_at = Set(chrono::Utc::now());
    update(db, active).await?;
    Ok(())
}
