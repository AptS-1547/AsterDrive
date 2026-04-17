//! 事务封装 helper。
//!
//! ## 模式说明
//!
//! 当前 service 层事务的标准模式：
//! ```ignore
//! let txn = db.begin().await?;
//! repo::operation(&txn, ...).await?;
//! repo::another_operation(&txn, ...).await?;
//! txn.commit().await?;
//! ```
//!
//! 如果将来需要统一的 `#[transaction]` 风格封装，可在此模块实现。

use crate::errors::Result;

/// Begin 并返回事务，供调用方处理 commit/rollback。
///
/// 用途：统一 `begin` 的错误映射（`map_err(AsterError::from)`）。
pub async fn begin<C: sea_orm::TransactionTrait>(db: &C) -> Result<C::Transaction> {
    db.begin().await.map_err(Into::into)
}
