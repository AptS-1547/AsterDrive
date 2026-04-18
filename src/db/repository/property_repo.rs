//! 仓储模块：`property_repo`。

use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, Set, TryInsertResult,
};

use crate::entities::entity_property::{self, Entity as EntityProperty};
use crate::errors::{AsterError, Result};
use crate::types::EntityType;

/// 查询实体的所有属性
pub async fn find_by_entity<C: ConnectionTrait>(
    db: &C,
    entity_type: EntityType,
    entity_id: i64,
) -> Result<Vec<entity_property::Model>> {
    EntityProperty::find()
        .filter(entity_property::Column::EntityType.eq(entity_type))
        .filter(entity_property::Column::EntityId.eq(entity_id))
        .all(db)
        .await
        .map_err(AsterError::from)
}

/// 插入或更新属性
pub async fn upsert<C: ConnectionTrait>(
    db: &C,
    entity_type: EntityType,
    entity_id: i64,
    namespace: &str,
    name: &str,
    value: Option<&str>,
) -> Result<entity_property::Model> {
    let value_owned = value.map(|v| v.to_string());
    let inserted = match EntityProperty::insert(entity_property::ActiveModel {
        entity_type: Set(entity_type),
        entity_id: Set(entity_id),
        namespace: Set(namespace.to_string()),
        name: Set(name.to_string()),
        value: Set(value_owned.clone()),
        ..Default::default()
    })
    .on_conflict_do_nothing_on([
        entity_property::Column::EntityType,
        entity_property::Column::EntityId,
        entity_property::Column::Namespace,
        entity_property::Column::Name,
    ])
    .exec(db)
    .await
    .map_err(AsterError::from)?
    {
        TryInsertResult::Inserted(_) => true,
        TryInsertResult::Conflicted => false,
        TryInsertResult::Empty => {
            return Err(AsterError::internal_error(
                "entity property upsert produced empty insert result",
            ));
        }
    };

    if !inserted {
        let result = EntityProperty::update_many()
            .col_expr(
                entity_property::Column::Value,
                sea_orm::sea_query::Expr::value(value_owned.clone()),
            )
            .filter(entity_property::Column::EntityType.eq(entity_type))
            .filter(entity_property::Column::EntityId.eq(entity_id))
            .filter(entity_property::Column::Namespace.eq(namespace))
            .filter(entity_property::Column::Name.eq(name))
            .exec(db)
            .await
            .map_err(AsterError::from)?;

        if result.rows_affected == 0 {
            return Err(AsterError::internal_error(format!(
                "entity property upsert update affected 0 rows for {entity_type:?}#{entity_id} {namespace}:{name}"
            )));
        }
    }

    EntityProperty::find()
        .filter(entity_property::Column::EntityType.eq(entity_type))
        .filter(entity_property::Column::EntityId.eq(entity_id))
        .filter(entity_property::Column::Namespace.eq(namespace))
        .filter(entity_property::Column::Name.eq(name))
        .one(db)
        .await
        .map_err(AsterError::from)?
        .ok_or_else(|| {
            AsterError::internal_error(format!(
                "entity property upsert could not reload row for {entity_type:?}#{entity_id} {namespace}:{name}"
            ))
        })
}

/// 删除单个属性
pub async fn delete_prop<C: ConnectionTrait>(
    db: &C,
    entity_type: EntityType,
    entity_id: i64,
    namespace: &str,
    name: &str,
) -> Result<()> {
    EntityProperty::delete_many()
        .filter(entity_property::Column::EntityType.eq(entity_type))
        .filter(entity_property::Column::EntityId.eq(entity_id))
        .filter(entity_property::Column::Namespace.eq(namespace))
        .filter(entity_property::Column::Name.eq(name))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 删除实体的所有属性（实体删除时级联清理）
pub async fn delete_all_for_entity<C: ConnectionTrait>(
    db: &C,
    entity_type: EntityType,
    entity_id: i64,
) -> Result<()> {
    EntityProperty::delete_many()
        .filter(entity_property::Column::EntityType.eq(entity_type))
        .filter(entity_property::Column::EntityId.eq(entity_id))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 批量删除多个实体的所有属性
pub async fn delete_all_for_entities<C: ConnectionTrait>(
    db: &C,
    entity_type: EntityType,
    entity_ids: &[i64],
) -> Result<()> {
    if entity_ids.is_empty() {
        return Ok(());
    }
    EntityProperty::delete_many()
        .filter(entity_property::Column::EntityType.eq(entity_type))
        .filter(entity_property::Column::EntityId.is_in(entity_ids.to_vec()))
        .exec(db)
        .await
        .map_err(AsterError::from)?;
    Ok(())
}

/// 检查实体是否有自定义属性
pub async fn has_properties<C: ConnectionTrait>(
    db: &C,
    entity_type: EntityType,
    entity_id: i64,
) -> Result<bool> {
    let count = EntityProperty::find()
        .filter(entity_property::Column::EntityType.eq(entity_type))
        .filter(entity_property::Column::EntityId.eq(entity_id))
        .count(db)
        .await
        .map_err(AsterError::from)?;
    Ok(count > 0)
}
