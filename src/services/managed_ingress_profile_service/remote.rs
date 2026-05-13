use crate::errors::Result;
use crate::runtime::PrimaryRuntimeState;
use crate::services::managed_follower_service;
use crate::storage::remote_protocol::{
    RemoteCreateIngressProfileRequest, RemoteIngressProfileInfo, RemoteStorageClient,
    RemoteUpdateIngressProfileRequest,
};

pub async fn list_remote<S: PrimaryRuntimeState>(
    state: &S,
    remote_node_id: i64,
) -> Result<Vec<RemoteIngressProfileInfo>> {
    remote_client_for_node(state, remote_node_id)
        .await?
        .list_ingress_profiles()
        .await
}

pub async fn create_remote<S: PrimaryRuntimeState>(
    state: &S,
    remote_node_id: i64,
    input: RemoteCreateIngressProfileRequest,
) -> Result<RemoteIngressProfileInfo> {
    remote_client_for_node(state, remote_node_id)
        .await?
        .create_ingress_profile(&input)
        .await
}

pub async fn update_remote<S: PrimaryRuntimeState>(
    state: &S,
    remote_node_id: i64,
    profile_key: &str,
    input: RemoteUpdateIngressProfileRequest,
) -> Result<RemoteIngressProfileInfo> {
    remote_client_for_node(state, remote_node_id)
        .await?
        .update_ingress_profile(profile_key, &input)
        .await
}

pub async fn delete_remote<S: PrimaryRuntimeState>(
    state: &S,
    remote_node_id: i64,
    profile_key: &str,
) -> Result<()> {
    remote_client_for_node(state, remote_node_id)
        .await?
        .delete_ingress_profile(profile_key)
        .await
}

async fn remote_client_for_node<S: PrimaryRuntimeState>(
    state: &S,
    remote_node_id: i64,
) -> Result<RemoteStorageClient> {
    let node =
        managed_follower_service::require_completed_enrollment(state, remote_node_id).await?;
    RemoteStorageClient::new(&node.base_url, &node.access_key, &node.secret_key)
}
