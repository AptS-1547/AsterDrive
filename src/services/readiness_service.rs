//! 服务模块：`readiness_service`。

use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::{PrimaryAppState, FollowerRuntimeState};
use sea_orm::DatabaseConnection;

pub async fn ping_database(db: &DatabaseConnection) -> Result<()> {
    db.ping()
        .await
        .map_aster_err(AsterError::database_operation)
}

pub async fn check_primary_ready(state: &PrimaryAppState) -> Result<()> {
    crate::services::policy_service::test_default_connection(state).await
}

pub async fn check_follower_ready<S: FollowerRuntimeState>(state: &S) -> Result<()> {
    crate::services::master_binding_service::assert_follower_ready(state).await
}
