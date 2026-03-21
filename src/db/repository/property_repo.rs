use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};

use crate::entities::entity_property::{self, Entity as EntityProperty};
use crate::errors::{AsterError, Result};
use crate::types::EntityType;

/// 查询实体的所有属性
pub async fn find_by_entity(
    db: &DatabaseConnection,
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
pub async fn upsert(
    db: &DatabaseConnection,
    entity_type: EntityType,
    entity_id: i64,
    namespace: &str,
    name: &str,
    value: Option<&str>,
) -> Result<entity_property::Model> {
    // 先查是否存在
    let existing = EntityProperty::find()
        .filter(entity_property::Column::EntityType.eq(entity_type))
        .filter(entity_property::Column::EntityId.eq(entity_id))
        .filter(entity_property::Column::Namespace.eq(namespace))
        .filter(entity_property::Column::Name.eq(name))
        .one(db)
        .await
        .map_err(AsterError::from)?;

    if let Some(existing) = existing {
        let mut active: entity_property::ActiveModel = existing.into();
        active.value = Set(value.map(|v| v.to_string()));
        active.update(db).await.map_err(AsterError::from)
    } else {
        let model = entity_property::ActiveModel {
            entity_type: Set(entity_type),
            entity_id: Set(entity_id),
            namespace: Set(namespace.to_string()),
            name: Set(name.to_string()),
            value: Set(value.map(|v| v.to_string())),
            ..Default::default()
        };
        model.insert(db).await.map_err(AsterError::from)
    }
}

/// 删除单个属性
pub async fn delete_prop(
    db: &DatabaseConnection,
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
pub async fn delete_all_for_entity(
    db: &DatabaseConnection,
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

/// 检查实体是否有自定义属性
pub async fn has_properties(
    db: &DatabaseConnection,
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
