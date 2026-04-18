//! 仓储模块：`pagination_repo`。

use sea_orm::{ConnectionTrait, EntityTrait, PaginatorTrait, QuerySelect, Select};

use crate::errors::{AsterError, Result};

pub async fn fetch_offset_page<C, E>(
    db: &C,
    query: Select<E>,
    limit: u64,
    offset: u64,
) -> Result<(Vec<E::Model>, u64)>
where
    C: ConnectionTrait,
    E: EntityTrait,
    Select<E>: QuerySelect,
    for<'db> Select<E>: PaginatorTrait<'db, C>,
{
    let total = query.clone().count(db).await.map_err(AsterError::from)?;
    let items = query
        .limit(limit)
        .offset(offset)
        .all(db)
        .await
        .map_err(AsterError::from)?;
    Ok((items, total))
}
