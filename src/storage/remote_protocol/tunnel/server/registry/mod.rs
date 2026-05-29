use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use sea_orm::DatabaseConnection;
use tokio::sync::Notify;

use crate::entities::managed_follower;
use crate::storage::error::{StorageErrorKind, storage_driver_error};

mod broker;
mod headers;
mod persistence;
mod polling;
mod streaming;

pub use broker::{
    RemoteTunnelBroker, RemoteTunnelHttpResponse, RemoteTunnelStreamHttpResponse,
};

use persistence::persist_tunnel_error;
use polling::{PendingTunnelResponse, RemoteTunnelConnection};
use streaming::{PendingStreamResponse, StreamingTunnelLane};

const REMOTE_TUNNEL_CONNECT_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const REMOTE_TUNNEL_REQUEST_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const REMOTE_TUNNEL_STREAM_CHANNEL_CAPACITY: usize = 16;

#[derive(Default)]
pub struct RemoteTunnelRegistry {
    connections: DashMap<String, RemoteTunnelConnection>,
    stream_lanes: DashMap<String, Vec<Arc<StreamingTunnelLane>>>,
    pending: DashMap<String, PendingTunnelResponse>,
    stream_pending: DashMap<String, PendingStreamResponse>,
    last_errors: DashMap<i64, String>,
    persistence_db: parking_lot::RwLock<Option<DatabaseConnection>>,
    connection_notify: Notify,
}

impl RemoteTunnelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_persistence_db(&self, db: DatabaseConnection) {
        *self.persistence_db.write() = Some(db);
    }

    pub fn is_online(&self, remote_node: &managed_follower::Model) -> bool {
        self.connections.contains_key(&remote_node.access_key)
            || self
                .stream_lanes
                .get(&remote_node.access_key)
                .map(|lanes| {
                    lanes
                        .iter()
                        .any(|lane| lane.remote_node_id == remote_node.id)
                })
                .unwrap_or(false)
            || self
                .pending
                .iter()
                .any(|entry| entry.remote_node_id == remote_node.id)
            || self
                .stream_pending
                .iter()
                .any(|entry| entry.remote_node_id == remote_node.id)
    }

    pub fn last_error(&self, remote_node_id: i64) -> Option<String> {
        self.last_errors
            .get(&remote_node_id)
            .map(|entry| entry.value().clone())
    }

    fn record_error(&self, remote_node_id: i64, error: impl Into<String>) {
        let error = error.into();
        if error.trim().is_empty() {
            self.clear_error(remote_node_id);
        } else {
            self.last_errors.insert(remote_node_id, error);
            self.persist_error(remote_node_id);
        }
    }

    pub(super) fn clear_error(&self, remote_node_id: i64) {
        if self.last_errors.remove(&remote_node_id).is_some() {
            self.persist_error(remote_node_id);
        }
    }

    fn persist_error(&self, remote_node_id: i64) {
        let Some(db) = self.persistence_db.read().clone() else {
            return;
        };
        let error = self.last_error(remote_node_id).unwrap_or_default();
        tokio::spawn(async move {
            if let Err(persist_error) = persist_tunnel_error(&db, remote_node_id, error).await {
                tracing::warn!(
                    remote_node_id,
                    "failed to persist reverse tunnel error state: {persist_error}"
                );
            }
        });
    }
}

pub fn reverse_tunnel_offline_error(remote_node_id: i64) -> crate::errors::AsterError {
    storage_driver_error(
        StorageErrorKind::Transient,
        format!("remote node #{remote_node_id} reverse tunnel is offline"),
    )
}
