//! 仓储模块：`master_binding_repo`。

use crate::entities::master_binding::{self, Entity as MasterBinding};
use crate::errors::{AsterError, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
};

pub async fn find_by_id<C: ConnectionTrait>(db: &C, id: i64) -> Result<master_binding::Model> {
    MasterBinding::find_by_id(id)
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| AsterError::record_not_found(format!("master_binding #{id}")))
}

pub async fn find_by_access_key<C: ConnectionTrait>(
    db: &C,
    access_key: &str,
) -> Result<Option<master_binding::Model>> {
    MasterBinding::find()
        .filter(master_binding::Column::AccessKey.eq(access_key))
        .one(db)
        .await
        .map_err(AsterError::from)
}

pub async fn find_all<C: ConnectionTrait>(db: &C) -> Result<Vec<master_binding::Model>> {
    MasterBinding::find()
        .order_by_desc(master_binding::Column::CreatedAt)
        .order_by_desc(master_binding::Column::Id)
        .all(db)
        .await
        .map_err(AsterError::from)
}

pub async fn create<C: ConnectionTrait>(
    db: &C,
    model: master_binding::ActiveModel,
) -> Result<master_binding::Model> {
    model.insert(db).await.map_err(AsterError::from)
}

pub async fn update<C: ConnectionTrait>(
    db: &C,
    model: master_binding::ActiveModel,
) -> Result<master_binding::Model> {
    model.update(db).await.map_err(AsterError::from)
}

pub async fn count_by_ingress_policy_id<C: ConnectionTrait>(
    db: &C,
    ingress_policy_id: i64,
) -> Result<u64> {
    MasterBinding::find()
        .filter(master_binding::Column::IngressPolicyId.eq(ingress_policy_id))
        .count(db)
        .await
        .map_err(AsterError::from)
}
