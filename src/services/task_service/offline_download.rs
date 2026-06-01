//! Offline download background task.

use std::collections::BTreeSet;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::time::{Duration as StdDuration, Instant};

use async_trait::async_trait;
use futures::StreamExt;
use governor::{Quota, RateLimiter};
use reqwest::header::{CONTENT_DISPOSITION, CONTENT_LENGTH};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::Digest;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::sleep;
use url::Url;

use crate::config::operations;
use crate::entities::{background_task, file};
use crate::errors::{AsterError, MapAsterErr, Result};
use crate::runtime::PrimaryAppState;
use crate::services::{
    folder_service,
    task_service::{
        TaskInfo, TaskLeaseGuard, cleanup_task_temp_dir_for_task, create_typed_task_record,
        get_task_in_scope, is_task_lease_lost, is_task_lease_renewal_timed_out, mark_task_progress,
        mark_task_succeeded, prepare_task_temp_dir, set_task_runtime_json,
        spec::{self, OfflineDownloadTask, decode_payload_as},
        steps::{
            TASK_STEP_DOWNLOAD_SOURCE, TASK_STEP_STORE_RESULT, TASK_STEP_VALIDATE_SOURCE,
            TASK_STEP_VERIFY_SOURCE, TASK_STEP_WAITING, parse_task_steps_json,
            set_task_step_active, set_task_step_succeeded,
        },
        task_scope,
        types::{
            CreateOfflineDownloadTaskParams, OfflineDownloadTaskPayload, OfflineDownloadTaskResult,
        },
    },
    workspace_storage_service::{self, WorkspaceStorageScope},
};
use crate::utils::numbers::{i64_to_u64, u64_to_i64, u128_to_u64, usize_to_i64, usize_to_u32};

const OFFLINE_DOWNLOAD_TEMP_FILE_NAME: &str = "source";
const PROGRESS_UPDATE_INTERVAL: StdDuration = StdDuration::from_millis(800);
const THROTTLED_DOWNLOAD_TIMEOUT_SLACK_SECS: u64 = 30;

pub(crate) async fn create_offline_download_task_in_scope(
    state: &PrimaryAppState,
    scope: WorkspaceStorageScope,
    params: CreateOfflineDownloadTaskParams,
) -> Result<TaskInfo> {
    workspace_storage_service::require_scope_access_with_db(state, state.writer_db(), scope)
        .await?;
    let request = normalize_offline_download_request(params)?;
    if let Some(target_folder_id) = request.target_folder_id {
        workspace_storage_service::verify_folder_access(state, scope, target_folder_id).await?;
    }

    let payload = OfflineDownloadTaskPayload {
        // TODO: Move raw source URLs out of persistent task payloads once
        // short-lived encrypted task secret storage exists.
        url: request.url.as_str().to_string(),
        filename: request.filename,
        target_folder_id: request.target_folder_id,
        expected_sha256: request.expected_sha256,
        source_display_url: Some(redact_url_for_display(&request.url)),
    };
    let display_name = match payload.filename.as_deref() {
        Some(filename) => format!("Import {filename} from link"),
        None => format!(
            "Import from {}",
            payload
                .source_display_url
                .as_deref()
                .unwrap_or("external link")
        ),
    };
    let task =
        create_typed_task_record::<OfflineDownloadTask>(state, scope, &display_name, &payload)
            .await?;
    get_task_in_scope(state, scope, task.id).await
}

pub(super) async fn process_offline_download_task(
    state: &PrimaryAppState,
    task: &background_task::Model,
    lease_guard: TaskLeaseGuard,
) -> Result<()> {
    let result = async {
        let scope = task_scope(task)?;
        let payload = decode_payload_as::<OfflineDownloadTask>(task)?;
        let mut steps =
            parse_task_steps_json(task.steps_json.as_ref().map(|raw| raw.as_ref()), task.kind)?;
        set_task_step_succeeded(
            &mut steps,
            TASK_STEP_WAITING,
            Some("Worker claimed task"),
            None,
        )?;
        set_task_step_active(
            &mut steps,
            TASK_STEP_VALIDATE_SOURCE,
            Some("Validating source URL"),
            None,
        )?;
        mark_task_progress(
            state,
            &lease_guard,
            0,
            0,
            Some("Validating source URL"),
            &steps,
        )
        .await?;

        if let Some(target_folder_id) = payload.target_folder_id {
            workspace_storage_service::verify_folder_access(state, scope, target_folder_id).await?;
        }
        let source_url = parse_and_validate_source_url(&payload.url)?;
        let source_display_url = payload
            .source_display_url
            .clone()
            .unwrap_or_else(|| redact_url_for_display(&source_url));
        let task_temp_dir = prepare_task_temp_dir(state, lease_guard.lease()).await?;
        let temp_path = Path::new(&task_temp_dir).join(OFFLINE_DOWNLOAD_TEMP_FILE_NAME);
        let max_bytes = operations::offline_download_max_file_size_bytes(&state.runtime_config);
        let max_bytes_per_sec =
            operations::offline_download_max_bytes_per_sec(&state.runtime_config);
        let timeout = StdDuration::from_secs(
            operations::offline_download_request_timeout_secs(&state.runtime_config).max(1),
        );
        let timeout =
            effective_offline_download_request_timeout(timeout, max_bytes, max_bytes_per_sec)?;
        let mut engine = offline_download_engine(state, max_bytes, timeout)?;

        set_task_step_succeeded(
            &mut steps,
            TASK_STEP_VALIDATE_SOURCE,
            Some("Source URL accepted"),
            None,
        )?;
        set_task_step_active(
            &mut steps,
            TASK_STEP_DOWNLOAD_SOURCE,
            Some("Downloading source file"),
            None,
        )?;
        mark_task_progress(
            state,
            &lease_guard,
            0,
            0,
            Some("Downloading source file"),
            &steps,
        )
        .await?;

        let downloaded = engine
            .download(
                state,
                &lease_guard,
                OfflineDownloadStartRequest {
                    url: source_url,
                    temp_path: temp_path.clone(),
                    expected_sha256: payload.expected_sha256.clone(),
                    max_bytes_per_sec,
                    runtime_json: task
                        .runtime_json
                        .as_ref()
                        .map(|value| value.as_ref().to_string()),
                },
                &mut steps,
            )
            .await?;

        set_task_step_succeeded(
            &mut steps,
            TASK_STEP_DOWNLOAD_SOURCE,
            Some("Source file downloaded"),
            Some((downloaded.bytes_written, downloaded.progress_total())),
        )?;
        set_task_step_active(
            &mut steps,
            TASK_STEP_VERIFY_SOURCE,
            Some("Verifying downloaded file"),
            Some((downloaded.bytes_written, downloaded.progress_total())),
        )?;
        mark_task_progress(
            state,
            &lease_guard,
            downloaded.bytes_written,
            downloaded.progress_total(),
            Some("Verifying downloaded file"),
            &steps,
        )
        .await?;
        set_task_step_succeeded(
            &mut steps,
            TASK_STEP_VERIFY_SOURCE,
            Some("Downloaded file verified"),
            Some((downloaded.bytes_written, downloaded.progress_total())),
        )?;

        let filename = resolve_offline_download_filename(
            payload.filename.as_deref(),
            downloaded.response_filename.as_deref(),
            &downloaded.final_url,
        )?;
        set_task_step_active(
            &mut steps,
            TASK_STEP_STORE_RESULT,
            Some("Importing file to workspace"),
            Some((downloaded.bytes_written, downloaded.progress_total())),
        )?;
        mark_task_progress(
            state,
            &lease_guard,
            downloaded.bytes_written,
            downloaded.progress_total(),
            Some("Importing file to workspace"),
            &steps,
        )
        .await?;

        let stored = workspace_storage_service::store_from_temp_internal(
            state,
            workspace_storage_service::StoreFromTempParams::new(
                scope,
                payload.target_folder_id,
                &filename,
                &temp_path.to_string_lossy(),
                downloaded.bytes_written,
            ),
            workspace_storage_service::StoreFromTempHints {
                precomputed_hash: Some(&downloaded.sha256),
                ..Default::default()
            },
            workspace_storage_service::NewFileMode::ResolveUnique,
            true,
        )
        .await?;
        cleanup_task_temp_dir_for_task(state, task.id).await?;
        set_task_step_succeeded(
            &mut steps,
            TASK_STEP_STORE_RESULT,
            Some(&format!("Imported as {}", stored.name)),
            Some((downloaded.bytes_written, downloaded.progress_total())),
        )?;
        let result_json =
            spec::serialize_result::<OfflineDownloadTask>(&OfflineDownloadTaskResult {
                file_id: stored.id,
                file_name: stored.name.clone(),
                folder_id: stored.folder_id,
                file_path: build_download_result_path(state, scope, &stored).await?,
                source_display_url,
                content_length: downloaded.bytes_written,
                sha256: downloaded.sha256.clone(),
            })?;
        mark_task_succeeded(
            state,
            &lease_guard,
            Some(&result_json),
            downloaded.bytes_written,
            downloaded.progress_total(),
            Some("Offline download imported"),
            &steps,
        )
        .await
    }
    .await;

    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            if !is_task_lease_lost(&error)
                && !is_task_lease_renewal_timed_out(&error)
                && let Err(cleanup_error) = cleanup_task_temp_dir_for_task(state, task.id).await
            {
                tracing::warn!(
                    task_id = task.id,
                    "failed to cleanup offline download temp dir after error: {cleanup_error}"
                );
            }
            Err(error)
        }
    }
}

pub(super) struct OfflineDownloadRetryPolicy;

impl super::retry::TaskRetryPolicy for OfflineDownloadRetryPolicy {
    fn retry_class(error: &AsterError) -> super::retry::TaskRetryClass {
        match error {
            AsterError::ValidationError(_) | AsterError::FileTooLarge(_) => {
                super::retry::TaskRetryClass::Never
            }
            _ => super::retry::default_retry_class(error),
        }
    }
}

#[derive(Debug)]
struct NormalizedOfflineDownloadRequest {
    url: Url,
    filename: Option<String>,
    target_folder_id: Option<i64>,
    expected_sha256: Option<String>,
}

#[derive(Debug)]
struct OfflineDownloadStartRequest {
    url: Url,
    temp_path: PathBuf,
    expected_sha256: Option<String>,
    max_bytes_per_sec: Option<u64>,
    runtime_json: Option<String>,
}

#[derive(Debug)]
struct OfflineDownloadComplete {
    final_url: Url,
    response_filename: Option<String>,
    bytes_written: i64,
    sha256: String,
    declared_content_length: Option<i64>,
}

impl OfflineDownloadComplete {
    fn progress_total(&self) -> i64 {
        self.declared_content_length
            .unwrap_or(self.bytes_written)
            .max(self.bytes_written)
    }
}

struct BuiltinHttpOfflineDownloadEngine {
    max_bytes: i64,
    request_timeout: StdDuration,
}

impl BuiltinHttpOfflineDownloadEngine {
    fn new(max_bytes: i64, request_timeout: StdDuration) -> Self {
        Self {
            max_bytes,
            request_timeout,
        }
    }
}

fn offline_download_engine(
    state: &PrimaryAppState,
    max_bytes: i64,
    timeout: StdDuration,
) -> Result<Box<dyn OfflineDownloadEngine + Send>> {
    match operations::offline_download_engine(&state.runtime_config) {
        operations::OfflineDownloadEngine::Builtin => Ok(Box::new(
            BuiltinHttpOfflineDownloadEngine::new(max_bytes, timeout),
        )),
        operations::OfflineDownloadEngine::Aria2 => Ok(Box::new(
            Aria2OfflineDownloadEngine::from_runtime_config(state, max_bytes, timeout)?,
        )),
    }
}

#[async_trait]
trait OfflineDownloadEngine {
    async fn download(
        &mut self,
        state: &PrimaryAppState,
        lease_guard: &TaskLeaseGuard,
        request: OfflineDownloadStartRequest,
        steps: &mut [super::TaskStepInfo],
    ) -> Result<OfflineDownloadComplete>;
}

#[async_trait]
impl OfflineDownloadEngine for BuiltinHttpOfflineDownloadEngine {
    async fn download(
        &mut self,
        state: &PrimaryAppState,
        lease_guard: &TaskLeaseGuard,
        request: OfflineDownloadStartRequest,
        steps: &mut [super::TaskStepInfo],
    ) -> Result<OfflineDownloadComplete> {
        let resolved = resolve_source_host(&request.url).await?;
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(self.request_timeout)
            .user_agent(crate::utils::OUTBOUND_HTTP_USER_AGENT)
            .resolve_to_addrs(&resolved.domain, &resolved.socket_addrs)
            .build()
            .map_aster_err_ctx(
                "build offline download HTTP client",
                AsterError::internal_error,
            )?;
        let response = client
            .get(request.url.clone())
            .send()
            .await
            .map_aster_err_ctx("request offline download source", transient_storage_error)?;
        let status = response.status();
        if status.is_redirection() {
            return Err(AsterError::validation_error(
                "offline download redirects are not supported",
            ));
        }
        if !status.is_success() {
            return Err(AsterError::storage_driver_error(format!(
                "transient: offline download source returned HTTP {status}"
            )));
        }

        let declared_content_length = declared_content_length(response.headers())?;
        if let Some(length) = declared_content_length {
            ensure_download_size_allowed(length, self.max_bytes)?;
        }
        let response_filename = response_filename(response.headers());
        let final_url = response.url().clone();
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(&request.temp_path)
            .await
            .map_aster_err_ctx(
                "create offline download temp file",
                AsterError::storage_driver_error,
            )?;
        let mut hasher = crate::utils::hash::new_sha256();
        let mut written = 0_i64;
        let progress_total = declared_content_length.unwrap_or(0).max(0);
        let mut last_progress = Instant::now()
            .checked_sub(PROGRESS_UPDATE_INTERVAL)
            .unwrap_or_else(Instant::now);
        let rate_limiter = OfflineDownloadRateLimiter::new(request.max_bytes_per_sec);

        while let Some(chunk) = stream.next().await {
            lease_guard.ensure_active()?;
            let chunk =
                chunk.map_aster_err_ctx("read offline download body", transient_storage_error)?;
            let chunk_len = usize_to_i64(chunk.len(), "offline download chunk size")?;
            written = written.checked_add(chunk_len).ok_or_else(|| {
                AsterError::file_too_large("offline download size exceeds supported range")
            })?;
            ensure_download_size_allowed(written, self.max_bytes)?;
            file.write_all(&chunk).await.map_aster_err_ctx(
                "write offline download temp file",
                AsterError::storage_driver_error,
            )?;
            hasher.update(&chunk);
            OfflineDownloadRateLimiter::throttle(rate_limiter.as_ref(), chunk.len(), lease_guard)
                .await?;

            if last_progress.elapsed() >= PROGRESS_UPDATE_INTERVAL {
                let status_text = format!("Downloaded {written} bytes");
                set_task_step_active(
                    steps,
                    TASK_STEP_DOWNLOAD_SOURCE,
                    Some(&status_text),
                    Some((written, progress_total.max(written))),
                )?;
                mark_task_progress(
                    state,
                    lease_guard,
                    written,
                    progress_total.max(written),
                    Some(&status_text),
                    steps,
                )
                .await?;
                last_progress = Instant::now();
            }
        }

        file.flush().await.map_aster_err_ctx(
            "flush offline download temp file",
            AsterError::storage_driver_error,
        )?;
        file.sync_all().await.map_aster_err_ctx(
            "sync offline download temp file",
            AsterError::storage_driver_error,
        )?;
        if let Some(length) = declared_content_length
            && written != length
        {
            return Err(AsterError::storage_driver_error(format!(
                "transient: offline download size mismatch: declared {length}, received {written}"
            )));
        }
        if written <= 0 {
            return Err(AsterError::validation_error(
                "offline download source returned an empty file",
            ));
        }
        let sha256 = crate::utils::hash::sha256_digest_to_hex(&hasher.finalize());
        verify_expected_sha256(request.expected_sha256.as_deref(), &sha256)?;

        Ok(OfflineDownloadComplete {
            final_url,
            response_filename,
            bytes_written: written,
            sha256,
            declared_content_length,
        })
    }
}

struct Aria2OfflineDownloadEngine {
    max_bytes: i64,
    download_timeout: StdDuration,
    client: Aria2RpcClient,
    split: u64,
    max_connection_per_server: u64,
    lowest_speed_limit_bytes_per_sec: Option<u64>,
}

impl Aria2OfflineDownloadEngine {
    fn from_runtime_config(
        state: &PrimaryAppState,
        max_bytes: i64,
        download_timeout: StdDuration,
    ) -> Result<Self> {
        let rpc_url = operations::offline_download_aria2_rpc_url(&state.runtime_config)
            .ok_or_else(|| {
                AsterError::validation_error(
                    "offline_download_aria2_rpc_url is required when offline_download_engine is aria2",
                )
            })?;
        let rpc_timeout = StdDuration::from_secs(
            operations::offline_download_aria2_request_timeout_secs(&state.runtime_config).max(1),
        );
        Ok(Self {
            max_bytes,
            download_timeout,
            client: Aria2RpcClient::new(
                &rpc_url,
                operations::offline_download_aria2_rpc_secret(&state.runtime_config),
                rpc_timeout,
            )?,
            split: operations::offline_download_aria2_split(&state.runtime_config),
            max_connection_per_server: operations::offline_download_aria2_max_connection_per_server(
                &state.runtime_config,
            ),
            lowest_speed_limit_bytes_per_sec:
                operations::offline_download_aria2_lowest_speed_limit_bytes_per_sec(
                    &state.runtime_config,
                ),
        })
    }

    fn options(&self, request: &OfflineDownloadStartRequest) -> Aria2AddUriOptions {
        let dir = request
            .temp_path
            .parent()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default();
        let out = request
            .temp_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| OFFLINE_DOWNLOAD_TEMP_FILE_NAME.to_string());

        Aria2AddUriOptions {
            dir,
            out,
            allow_overwrite: "true".to_string(),
            auto_file_renaming: "false".to_string(),
            follow_torrent: "false".to_string(),
            follow_metalink: "false".to_string(),
            max_redirect: "0".to_string(),
            user_agent: crate::utils::OUTBOUND_HTTP_USER_AGENT.to_string(),
            split: self.split.to_string(),
            max_connection_per_server: self.max_connection_per_server.to_string(),
            lowest_speed_limit: self
                .lowest_speed_limit_bytes_per_sec
                .map(|limit| limit.to_string()),
            max_download_limit: request.max_bytes_per_sec.map(|limit| limit.to_string()),
        }
    }
}

#[async_trait]
impl OfflineDownloadEngine for Aria2OfflineDownloadEngine {
    async fn download(
        &mut self,
        state: &PrimaryAppState,
        lease_guard: &TaskLeaseGuard,
        request: OfflineDownloadStartRequest,
        steps: &mut [super::TaskStepInfo],
    ) -> Result<OfflineDownloadComplete> {
        resolve_source_host(&request.url).await?;
        let mut runtime_state =
            decode_offline_download_runtime_state(request.runtime_json.as_deref());
        if let Some(previous) = runtime_state.aria2.as_ref()
            && previous.processing_token != lease_guard.lease().processing_token
            && let Err(error) = self.client.force_remove(&previous.gid).await
        {
            tracing::warn!(
                task_id = lease_guard.lease().task_id,
                gid = previous.gid,
                "failed to cleanup stale aria2 offline download before retry: {error}"
            );
        }

        let gid = self
            .client
            .add_uri(request.url.as_str(), self.options(&request))
            .await?;
        runtime_state.aria2 = Some(Aria2TaskRuntime {
            gid: gid.clone(),
            processing_token: lease_guard.lease().processing_token,
        });
        persist_offline_download_runtime_state(state, lease_guard, &runtime_state).await?;

        let result = self
            .poll_until_complete(state, lease_guard, &request, steps, &gid)
            .await;
        if result.is_err()
            && let Err(cleanup_error) = self.client.force_remove(&gid).await
        {
            tracing::warn!(
                task_id = lease_guard.lease().task_id,
                gid,
                "failed to cleanup aria2 offline download after error: {cleanup_error}"
            );
        }
        result
    }
}

impl Aria2OfflineDownloadEngine {
    async fn poll_until_complete(
        &self,
        state: &PrimaryAppState,
        lease_guard: &TaskLeaseGuard,
        request: &OfflineDownloadStartRequest,
        steps: &mut [super::TaskStepInfo],
        gid: &str,
    ) -> Result<OfflineDownloadComplete> {
        let started_at = Instant::now();
        let mut last_progress = Instant::now()
            .checked_sub(PROGRESS_UPDATE_INTERVAL)
            .unwrap_or_else(Instant::now);
        let mut declared_content_length = None;

        loop {
            lease_guard.ensure_active()?;
            if started_at.elapsed() > self.download_timeout {
                return Err(AsterError::storage_driver_error(
                    "transient: aria2 offline download timed out",
                ));
            }

            let status = self.client.tell_status(gid).await?;
            let completed = parse_aria2_length(&status.completed_length, "aria2 completedLength")?;
            ensure_download_size_allowed(completed, self.max_bytes)?;
            let total = parse_aria2_length(&status.total_length, "aria2 totalLength")?;
            if total > 0 {
                ensure_download_size_allowed(total, self.max_bytes)?;
                declared_content_length = Some(total);
            }

            match &status.status {
                Aria2DownloadStatus::Complete => {
                    break;
                }
                Aria2DownloadStatus::Active
                | Aria2DownloadStatus::Waiting
                | Aria2DownloadStatus::Paused => {
                    if last_progress.elapsed() >= PROGRESS_UPDATE_INTERVAL {
                        let progress_total = total.max(completed);
                        let status_text = format!("Downloaded {completed} bytes");
                        set_task_step_active(
                            steps,
                            TASK_STEP_DOWNLOAD_SOURCE,
                            Some(&status_text),
                            Some((completed, progress_total)),
                        )?;
                        mark_task_progress(
                            state,
                            lease_guard,
                            completed,
                            progress_total,
                            Some(&status_text),
                            steps,
                        )
                        .await?;
                        last_progress = Instant::now();
                    }
                }
                Aria2DownloadStatus::Error => {
                    return Err(AsterError::storage_driver_error(format!(
                        "transient: aria2 offline download failed: {}",
                        status.error_summary()
                    )));
                }
                Aria2DownloadStatus::Removed => {
                    return Err(AsterError::storage_driver_error(
                        "transient: aria2 offline download was removed",
                    ));
                }
                Aria2DownloadStatus::Unknown(other) => {
                    return Err(AsterError::storage_driver_error(format!(
                        "transient: aria2 offline download entered unsupported status {other}"
                    )));
                }
            }

            sleep(PROGRESS_UPDATE_INTERVAL).await;
        }

        let bytes_written = downloaded_file_size(&request.temp_path).await?;
        ensure_download_size_allowed(bytes_written, self.max_bytes)?;
        if bytes_written <= 0 {
            return Err(AsterError::validation_error(
                "offline download source returned an empty file",
            ));
        }
        if let Some(length) = declared_content_length
            && bytes_written != length
        {
            return Err(AsterError::storage_driver_error(format!(
                "transient: offline download size mismatch: declared {length}, received {bytes_written}"
            )));
        }
        let sha256 = sha256_file(&request.temp_path).await?;
        verify_expected_sha256(request.expected_sha256.as_deref(), &sha256)?;
        if let Err(error) = self.client.remove_download_result(gid).await {
            tracing::warn!(
                gid,
                "failed to remove completed aria2 download result: {error}"
            );
        }

        Ok(OfflineDownloadComplete {
            final_url: request.url.clone(),
            response_filename: None,
            bytes_written,
            sha256,
            declared_content_length,
        })
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct OfflineDownloadRuntimeState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    aria2: Option<Aria2TaskRuntime>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Aria2TaskRuntime {
    gid: String,
    processing_token: i64,
}

fn decode_offline_download_runtime_state(raw: Option<&str>) -> OfflineDownloadRuntimeState {
    let Some(raw) = raw else {
        return OfflineDownloadRuntimeState::default();
    };
    if raw.trim().is_empty() {
        return OfflineDownloadRuntimeState::default();
    }
    serde_json::from_str(raw).unwrap_or_else(|error| {
        tracing::warn!("invalid offline download runtime_json; ignoring it: {error}");
        OfflineDownloadRuntimeState::default()
    })
}

async fn persist_offline_download_runtime_state(
    state: &PrimaryAppState,
    lease_guard: &TaskLeaseGuard,
    runtime_state: &OfflineDownloadRuntimeState,
) -> Result<()> {
    let runtime_json = serde_json::to_string(runtime_state).map_aster_err_ctx(
        "serialize offline download runtime state",
        AsterError::internal_error,
    )?;
    set_task_runtime_json(state, lease_guard, Some(&runtime_json)).await
}

struct Aria2RpcClient {
    endpoint: Url,
    secret: Option<String>,
    client: reqwest::Client,
}

impl Aria2RpcClient {
    fn new(endpoint: &str, secret: Option<String>, timeout: StdDuration) -> Result<Self> {
        let endpoint = Url::parse(endpoint)
            .map_aster_err_ctx("parse aria2 RPC URL", AsterError::validation_error)?;
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(crate::utils::OUTBOUND_HTTP_USER_AGENT)
            .build()
            .map_aster_err_ctx("build aria2 RPC HTTP client", AsterError::internal_error)?;
        Ok(Self {
            endpoint,
            secret,
            client,
        })
    }

    async fn add_uri(&self, source_url: &str, options: Aria2AddUriOptions) -> Result<String> {
        self.call(
            Aria2RpcMethod::AddUri,
            self.params(
                Aria2RpcParam::Uris(vec![source_url.to_string()]),
                Some(Aria2RpcParam::AddUriOptions(options)),
            ),
        )
        .await
    }

    async fn tell_status(&self, gid: &str) -> Result<Aria2TellStatus> {
        self.call(
            Aria2RpcMethod::TellStatus,
            self.params(
                Aria2RpcParam::String(gid.to_string()),
                Some(Aria2RpcParam::TellStatusKeys(vec![
                    Aria2TellStatusKey::Gid,
                    Aria2TellStatusKey::Status,
                    Aria2TellStatusKey::TotalLength,
                    Aria2TellStatusKey::CompletedLength,
                    Aria2TellStatusKey::DownloadSpeed,
                    Aria2TellStatusKey::ErrorCode,
                    Aria2TellStatusKey::ErrorMessage,
                ])),
            ),
        )
        .await
    }

    async fn force_remove(&self, gid: &str) -> Result<()> {
        self.call_ignore_missing(Aria2RpcMethod::ForceRemove, gid)
            .await
    }

    async fn remove_download_result(&self, gid: &str) -> Result<()> {
        self.call_ignore_missing(Aria2RpcMethod::RemoveDownloadResult, gid)
            .await
    }

    async fn call_ignore_missing(&self, method: Aria2RpcMethod, gid: &str) -> Result<()> {
        match self
            .call_raw::<String>(
                method,
                self.params(Aria2RpcParam::String(gid.to_string()), None),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(Aria2RpcCallError::Rpc { code: 1, .. }) => Ok(()),
            Err(error) => Err(error.into_aster_error()),
        }
    }

    fn params(&self, first: Aria2RpcParam, second: Option<Aria2RpcParam>) -> Vec<Aria2RpcParam> {
        let mut params = Vec::new();
        if let Some(secret) = self.secret.as_deref() {
            params.push(Aria2RpcParam::String(format!("token:{secret}")));
        }
        params.push(first);
        if let Some(second) = second {
            params.push(second);
        }
        params
    }

    async fn call<T: DeserializeOwned>(
        &self,
        method: Aria2RpcMethod,
        params: Vec<Aria2RpcParam>,
    ) -> Result<T> {
        self.call_raw(method, params)
            .await
            .map_err(Aria2RpcCallError::into_aster_error)
    }

    async fn call_raw<T: DeserializeOwned>(
        &self,
        method: Aria2RpcMethod,
        params: Vec<Aria2RpcParam>,
    ) -> std::result::Result<T, Aria2RpcCallError> {
        let request = Aria2JsonRpcRequest {
            jsonrpc: "2.0",
            id: format!("asterdrive-{}", crate::utils::id::new_uuid()),
            method: method.as_str(),
            params,
        };
        let response = self
            .client
            .post(self.endpoint.clone())
            .json(&request)
            .send()
            .await
            .map_err(|error| {
                Aria2RpcCallError::Transport(transient_storage_error(format!(
                    "send aria2 RPC request: {error}"
                )))
            })?;
        if !response.status().is_success() {
            return Err(Aria2RpcCallError::Transport(
                AsterError::storage_driver_error(format!(
                    "transient: aria2 RPC returned HTTP {}",
                    response.status()
                )),
            ));
        }
        let body = response
            .json::<Aria2JsonRpcResponse<T>>()
            .await
            .map_err(|error| {
                Aria2RpcCallError::Transport(transient_storage_error(format!(
                    "decode aria2 RPC response: {error}"
                )))
            })?;
        if let Some(error) = body.error {
            return Err(Aria2RpcCallError::Rpc {
                method: method.as_str(),
                code: error.code,
                message: error.message,
            });
        }
        body.result.ok_or_else(|| {
            Aria2RpcCallError::Transport(AsterError::storage_driver_error(format!(
                "transient: aria2 RPC {} returned no result",
                method.as_str()
            )))
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Aria2RpcMethod {
    AddUri,
    TellStatus,
    ForceRemove,
    RemoveDownloadResult,
}

impl Aria2RpcMethod {
    fn as_str(self) -> &'static str {
        match self {
            Self::AddUri => "aria2.addUri",
            Self::TellStatus => "aria2.tellStatus",
            Self::ForceRemove => "aria2.forceRemove",
            Self::RemoveDownloadResult => "aria2.removeDownloadResult",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum Aria2RpcParam {
    String(String),
    Uris(Vec<String>),
    AddUriOptions(Aria2AddUriOptions),
    TellStatusKeys(Vec<Aria2TellStatusKey>),
}

#[derive(Debug, Serialize)]
struct Aria2AddUriOptions {
    dir: String,
    out: String,
    #[serde(rename = "allow-overwrite")]
    allow_overwrite: String,
    #[serde(rename = "auto-file-renaming")]
    auto_file_renaming: String,
    #[serde(rename = "follow-torrent")]
    follow_torrent: String,
    #[serde(rename = "follow-metalink")]
    follow_metalink: String,
    #[serde(rename = "max-redirect")]
    max_redirect: String,
    #[serde(rename = "user-agent")]
    user_agent: String,
    split: String,
    #[serde(rename = "max-connection-per-server")]
    max_connection_per_server: String,
    #[serde(rename = "lowest-speed-limit", skip_serializing_if = "Option::is_none")]
    lowest_speed_limit: Option<String>,
    #[serde(rename = "max-download-limit", skip_serializing_if = "Option::is_none")]
    max_download_limit: Option<String>,
}

#[derive(Debug, Serialize)]
enum Aria2TellStatusKey {
    #[serde(rename = "gid")]
    Gid,
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "totalLength")]
    TotalLength,
    #[serde(rename = "completedLength")]
    CompletedLength,
    #[serde(rename = "downloadSpeed")]
    DownloadSpeed,
    #[serde(rename = "errorCode")]
    ErrorCode,
    #[serde(rename = "errorMessage")]
    ErrorMessage,
}

enum Aria2RpcCallError {
    Rpc {
        method: &'static str,
        code: i64,
        message: String,
    },
    Transport(AsterError),
}

impl Aria2RpcCallError {
    fn into_aster_error(self) -> AsterError {
        match self {
            Self::Rpc {
                method,
                code,
                message,
            } => AsterError::storage_driver_error(format!(
                "transient: aria2 RPC {method} failed with code {code}: {message}"
            )),
            Self::Transport(error) => error,
        }
    }
}

#[derive(Serialize)]
pub(super) struct Aria2JsonRpcRequest<'a> {
    jsonrpc: &'static str,
    id: String,
    method: &'a str,
    params: Vec<Aria2RpcParam>,
}

#[derive(Deserialize)]
struct Aria2JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<Aria2JsonRpcError>,
}

#[derive(Deserialize)]
struct Aria2JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct Aria2TellStatus {
    status: Aria2DownloadStatus,
    #[serde(rename = "totalLength", default)]
    total_length: String,
    #[serde(rename = "completedLength", default)]
    completed_length: String,
    #[serde(rename = "errorCode", default)]
    error_code: Option<String>,
    #[serde(rename = "errorMessage", default)]
    error_message: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum Aria2DownloadStatus {
    Active,
    Waiting,
    Paused,
    Error,
    Complete,
    Removed,
    Unknown(String),
}

impl<'de> Deserialize<'de> for Aria2DownloadStatus {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "active" => Self::Active,
            "waiting" => Self::Waiting,
            "paused" => Self::Paused,
            "error" => Self::Error,
            "complete" => Self::Complete,
            "removed" => Self::Removed,
            _ => Self::Unknown(value),
        })
    }
}

impl Aria2TellStatus {
    fn error_summary(&self) -> String {
        match (self.error_code.as_deref(), self.error_message.as_deref()) {
            (Some(code), Some(message)) if !message.is_empty() => {
                format!("{code}: {message}")
            }
            (Some(code), _) => code.to_string(),
            (_, Some(message)) if !message.is_empty() => message.to_string(),
            _ => "unknown error".to_string(),
        }
    }
}

fn parse_aria2_length(value: &str, field: &str) -> Result<i64> {
    if value.trim().is_empty() {
        return Ok(0);
    }
    let parsed = value
        .parse::<u64>()
        .map_aster_err_ctx(&format!("parse {field}"), AsterError::storage_driver_error)?;
    u64_to_i64(parsed, field)
}

async fn downloaded_file_size(path: &Path) -> Result<i64> {
    let metadata = tokio::fs::metadata(path).await.map_aster_err_ctx(
        "stat offline download temp file",
        AsterError::storage_driver_error,
    )?;
    u64_to_i64(metadata.len(), "offline download temp file size")
}

async fn sha256_file(path: &Path) -> Result<String> {
    let mut file = tokio::fs::File::open(path).await.map_aster_err_ctx(
        "open offline download temp file",
        AsterError::storage_driver_error,
    )?;
    let mut hasher = crate::utils::hash::new_sha256();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).await.map_aster_err_ctx(
            "read offline download temp file",
            AsterError::storage_driver_error,
        )?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(crate::utils::hash::sha256_digest_to_hex(&hasher.finalize()))
}

struct ResolvedSourceHost {
    domain: String,
    socket_addrs: Vec<SocketAddr>,
}

fn normalize_offline_download_request(
    params: CreateOfflineDownloadTaskParams,
) -> Result<NormalizedOfflineDownloadRequest> {
    let url = parse_and_validate_source_url(&params.url)?;
    let filename = match params.filename {
        Some(filename) => {
            let trimmed = filename.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(crate::utils::normalize_validate_name(trimmed)?)
            }
        }
        None => None,
    };
    let expected_sha256 = match params.expected_sha256 {
        Some(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            if normalized.is_empty() {
                None
            } else {
                validate_sha256_hex(&normalized)?;
                Some(normalized)
            }
        }
        None => None,
    };

    Ok(NormalizedOfflineDownloadRequest {
        url,
        filename,
        target_folder_id: params.target_folder_id,
        expected_sha256,
    })
}

fn parse_and_validate_source_url(raw: &str) -> Result<Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AsterError::validation_error("url cannot be empty"));
    }
    let url = Url::parse(trimmed)
        .map_aster_err_ctx("parse offline download url", AsterError::validation_error)?;
    match url.scheme() {
        "http" | "https" => {}
        _ => {
            return Err(AsterError::validation_error(
                "offline download only supports http and https URLs",
            ));
        }
    }
    if url.host_str().is_none() {
        return Err(AsterError::validation_error(
            "offline download url must include a host",
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AsterError::validation_error(
            "offline download url must not include credentials",
        ));
    }
    Ok(url)
}

fn effective_offline_download_request_timeout(
    configured_timeout: StdDuration,
    max_bytes: i64,
    max_bytes_per_sec: Option<u64>,
) -> Result<StdDuration> {
    let Some(max_bytes_per_sec) = max_bytes_per_sec else {
        return Ok(configured_timeout);
    };
    if max_bytes <= 0 || max_bytes_per_sec == 0 {
        return Ok(configured_timeout);
    }

    let max_bytes = i64_to_u64(max_bytes, "offline download max file size")?;
    let expected_secs = u128::from(max_bytes).div_ceil(u128::from(max_bytes_per_sec));
    let expected_secs =
        expected_secs.saturating_add(u128::from(THROTTLED_DOWNLOAD_TIMEOUT_SLACK_SECS));
    let expected_secs = u128_to_u64(expected_secs, "offline download effective timeout")?;
    Ok(configured_timeout.max(StdDuration::from_secs(expected_secs)))
}

async fn resolve_source_host(url: &Url) -> Result<ResolvedSourceHost> {
    let host = url
        .host_str()
        .ok_or_else(|| AsterError::validation_error("offline download url must include a host"))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| AsterError::validation_error("offline download url has no port"))?;
    let socket_addrs = tokio::net::lookup_host((host, port))
        .await
        .map_aster_err_ctx(
            "resolve offline download host",
            AsterError::validation_error,
        )?
        .collect::<Vec<_>>();
    if socket_addrs.is_empty() {
        return Err(AsterError::validation_error(
            "offline download host did not resolve to any address",
        ));
    }

    let mut unique_ips = BTreeSet::new();
    let mut safe_addrs = Vec::new();
    for socket_addr in socket_addrs {
        let ip = socket_addr.ip();
        if !unique_ips.insert(ip) {
            continue;
        }
        validate_public_download_ip(ip)?;
        safe_addrs.push(SocketAddr::new(ip, port));
    }

    Ok(ResolvedSourceHost {
        domain: host.to_ascii_lowercase(),
        socket_addrs: safe_addrs,
    })
}

fn validate_public_download_ip(ip: IpAddr) -> Result<()> {
    let blocked = match ip {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            ip.is_unspecified()
                || ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_broadcast()
                || ip.is_documentation()
                || octets[0] == 0
                || octets[0] >= 240
                || octets == [169, 254, 169, 254]
                || (octets[0] == 100 && (octets[1] & 0xc0 == 0x40))
                || (octets[0] == 198 && (octets[1] == 18 || octets[1] == 19))
        }
        IpAddr::V6(ip) => {
            if let Some(mapped) = ip.to_ipv4_mapped() {
                return validate_public_download_ip(IpAddr::V4(mapped));
            }
            ip.is_unspecified()
                || ip.is_loopback()
                || ip.is_multicast()
                || ip.segments()[0] & 0xfe00 == 0xfc00
                || ip.segments()[0] & 0xffc0 == 0xfe80
                || ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8
        }
    };
    if blocked {
        return Err(AsterError::validation_error(
            "offline download host resolves to a blocked address",
        ));
    }
    Ok(())
}

struct OfflineDownloadRateLimiter {
    limiter: governor::DefaultDirectRateLimiter,
    max_batch_bytes: u32,
}

impl OfflineDownloadRateLimiter {
    fn new(max_bytes_per_sec: Option<u64>) -> Option<Self> {
        let max_bytes_per_sec = max_bytes_per_sec?;
        let max_batch_bytes = u32::try_from(max_bytes_per_sec).unwrap_or(u32::MAX);
        let max_batch_bytes = NonZeroU32::new(max_batch_bytes)?;
        Some(Self {
            limiter: RateLimiter::direct(Quota::per_second(max_batch_bytes)),
            max_batch_bytes: max_batch_bytes.get(),
        })
    }

    async fn throttle(
        limiter: Option<&Self>,
        chunk_len: usize,
        lease_guard: &TaskLeaseGuard,
    ) -> Result<()> {
        let Some(limiter) = limiter else {
            return Ok(());
        };
        let mut remaining = usize_to_u32(chunk_len, "offline download throttle chunk size")?;
        while remaining > 0 {
            lease_guard.ensure_active()?;
            let batch = remaining.min(limiter.max_batch_bytes);
            let batch = NonZeroU32::new(batch).ok_or_else(|| {
                AsterError::internal_error("offline download throttle batch cannot be zero")
            })?;
            limiter
                .limiter
                .until_n_ready(batch)
                .await
                .map_aster_err_ctx(
                    "reserve offline download throttle capacity",
                    AsterError::internal_error,
                )?;
            remaining -= batch.get();
        }
        Ok(())
    }
}

fn declared_content_length(headers: &reqwest::header::HeaderMap) -> Result<Option<i64>> {
    let Some(value) = headers.get(CONTENT_LENGTH) else {
        return Ok(None);
    };
    let Ok(raw) = value.to_str() else {
        return Ok(None);
    };
    let Ok(parsed) = raw.parse::<u64>() else {
        return Ok(None);
    };
    Ok(u64_to_i64(parsed, "offline download content length").ok())
}

fn response_filename(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get(CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .and_then(filename_from_content_disposition)
}

fn filename_from_content_disposition(raw: &str) -> Option<String> {
    let mut fallback = None;
    for part in raw.split(';').skip(1) {
        let (name, value) = part.trim().split_once('=')?;
        let name = name.trim();
        if name.eq_ignore_ascii_case("filename*") {
            if let Some(decoded) = decode_rfc5987_filename(value.trim())
                && let Ok(name) = crate::utils::normalize_validate_name(&decoded)
            {
                return Some(name);
            }
        } else if name.eq_ignore_ascii_case("filename") {
            let value = value.trim().trim_matches('"');
            if !value.is_empty()
                && let Ok(name) = crate::utils::normalize_validate_name(value)
            {
                fallback = Some(name);
            }
        }
    }
    fallback
}

fn decode_rfc5987_filename(raw: &str) -> Option<String> {
    let raw = raw.trim().trim_matches('"');
    let mut parts = raw.splitn(3, '\'');
    let charset = parts.next()?.trim();
    let _language = parts.next()?;
    let encoded = parts.next()?;
    let decoded_bytes = percent_encoding::percent_decode_str(encoded).collect::<Vec<u8>>();
    match charset.to_ascii_lowercase().as_str() {
        "utf-8" | "us-ascii" => String::from_utf8(decoded_bytes).ok(),
        _ => None,
    }
}

fn resolve_offline_download_filename(
    requested: Option<&str>,
    response: Option<&str>,
    url: &Url,
) -> Result<String> {
    if let Some(name) = requested {
        return crate::utils::normalize_validate_name(name);
    }
    if let Some(name) = response {
        return crate::utils::normalize_validate_name(name);
    }
    let from_path = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .and_then(|segment| {
            percent_encoding::percent_decode_str(segment)
                .decode_utf8()
                .ok()
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(name) = from_path
        && let Ok(name) = crate::utils::normalize_validate_name(&name)
    {
        return Ok(name);
    }
    Ok("download".to_string())
}

fn validate_sha256_hex(value: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AsterError::validation_error(
            "expected_sha256 must be a 64-character hex string",
        ));
    }
    Ok(())
}

fn verify_expected_sha256(expected: Option<&str>, actual: &str) -> Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    if expected != actual {
        return Err(AsterError::validation_error(format!(
            "offline download sha256 mismatch: expected {expected}, got {actual}"
        )));
    }
    Ok(())
}

fn ensure_download_size_allowed(size: i64, max_bytes: i64) -> Result<()> {
    if size > max_bytes {
        return Err(AsterError::file_too_large(format!(
            "offline download source size {size} exceeds server limit {max_bytes}"
        )));
    }
    Ok(())
}

fn transient_storage_error(message: impl Into<String>) -> AsterError {
    AsterError::storage_driver_error(format!("transient: {}", message.into()))
}

pub(super) fn redact_url_for_display(url: &Url) -> String {
    let mut redacted = url.clone();
    let _ = redacted.set_username("");
    let _ = redacted.set_password(None);
    redacted.set_query(None);
    redacted.set_fragment(None);
    redacted.to_string()
}

async fn build_download_result_path(
    state: &PrimaryAppState,
    _scope: WorkspaceStorageScope,
    file: &file::Model,
) -> Result<String> {
    let Some(folder_id) = file.folder_id else {
        return Ok(format!("/{}", file.name));
    };
    let paths = folder_service::build_folder_paths(state.writer_db(), &[folder_id]).await?;
    let folder_path = paths.get(&folder_id).cloned().unwrap_or_default();
    if folder_path.is_empty() || folder_path == "/" {
        Ok(format!("/{}", file.name))
    } else {
        Ok(format!(
            "{}/{}",
            folder_path.trim_end_matches('/'),
            file.name
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use serde_json::Value;
    use serde_json::json;

    use super::*;

    fn request(url: &str) -> CreateOfflineDownloadTaskParams {
        CreateOfflineDownloadTaskParams {
            url: url.to_string(),
            filename: None,
            target_folder_id: None,
            expected_sha256: None,
        }
    }

    #[test]
    fn redact_url_for_display_removes_sensitive_parts() {
        let url = Url::parse(
            "https://user:secret@example.com:8443/files/archive.zip?token=secret#download",
        )
        .unwrap();

        assert_eq!(
            redact_url_for_display(&url),
            "https://example.com:8443/files/archive.zip"
        );
    }

    #[test]
    fn parse_source_url_only_accepts_http_and_https() {
        assert!(parse_and_validate_source_url(" https://example.com/file ").is_ok());
        assert!(parse_and_validate_source_url("http://example.com/file").is_ok());
        assert!(parse_and_validate_source_url("ftp://example.com/file").is_err());
        assert!(parse_and_validate_source_url("file:///etc/passwd").is_err());
        assert!(parse_and_validate_source_url("https://").is_err());
        assert!(parse_and_validate_source_url("https://user@example.com/file").is_err());
        assert!(parse_and_validate_source_url("https://:secret@example.com/file").is_err());
    }

    #[test]
    fn effective_timeout_accounts_for_local_rate_limit() {
        let configured = StdDuration::from_secs(600);

        assert_eq!(
            effective_offline_download_request_timeout(configured, 1024 * 1024 * 1024, None)
                .unwrap(),
            configured
        );
        assert_eq!(
            effective_offline_download_request_timeout(
                configured,
                1024 * 1024 * 1024,
                Some(1024 * 1024)
            )
            .unwrap(),
            StdDuration::from_secs(1024 + THROTTLED_DOWNLOAD_TIMEOUT_SLACK_SECS)
        );
        assert_eq!(
            effective_offline_download_request_timeout(
                StdDuration::from_secs(1200),
                1024 * 1024 * 1024,
                Some(1024 * 1024)
            )
            .unwrap(),
            StdDuration::from_secs(1200)
        );
    }

    #[test]
    fn validate_public_download_ip_rejects_sensitive_ipv4_ranges() {
        for ip in [
            Ipv4Addr::new(0, 0, 0, 0),
            Ipv4Addr::new(100, 64, 0, 1),
            Ipv4Addr::new(100, 127, 255, 254),
            Ipv4Addr::new(10, 0, 0, 1),
            Ipv4Addr::new(127, 0, 0, 1),
            Ipv4Addr::new(198, 18, 0, 1),
            Ipv4Addr::new(198, 19, 255, 254),
            Ipv4Addr::new(169, 254, 1, 1),
            Ipv4Addr::new(169, 254, 169, 254),
            Ipv4Addr::new(172, 16, 0, 1),
            Ipv4Addr::new(192, 168, 1, 1),
            Ipv4Addr::new(224, 0, 0, 1),
            Ipv4Addr::new(240, 0, 0, 1),
        ] {
            assert!(
                validate_public_download_ip(IpAddr::V4(ip)).is_err(),
                "{ip} should be blocked"
            );
        }

        assert!(validate_public_download_ip(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))).is_ok());
    }

    #[test]
    fn validate_public_download_ip_rejects_sensitive_ipv6_ranges() {
        for ip in [
            Ipv6Addr::UNSPECIFIED,
            Ipv6Addr::LOCALHOST,
            "fc00::1".parse().unwrap(),
            "fd12:3456::1".parse().unwrap(),
            "fe80::1".parse().unwrap(),
            "ff02::1".parse().unwrap(),
            "2001:db8::1".parse().unwrap(),
            "::ffff:127.0.0.1".parse().unwrap(),
            "::ffff:10.0.0.1".parse().unwrap(),
            "::ffff:169.254.169.254".parse().unwrap(),
        ] {
            assert!(
                validate_public_download_ip(IpAddr::V6(ip)).is_err(),
                "{ip} should be blocked"
            );
        }

        assert!(
            validate_public_download_ip(IpAddr::V6(
                "2606:2800:220:1:248:1893:25c8:1946".parse().unwrap()
            ))
            .is_ok()
        );
    }

    #[test]
    fn offline_download_rate_limiter_uses_one_second_batch_cap() {
        assert!(OfflineDownloadRateLimiter::new(None).is_none());
        assert!(OfflineDownloadRateLimiter::new(Some(0)).is_none());

        let limiter = OfflineDownloadRateLimiter::new(Some(5)).unwrap();
        assert_eq!(limiter.max_batch_bytes, 5);

        let large = OfflineDownloadRateLimiter::new(Some(u64::MAX)).unwrap();
        assert_eq!(large.max_batch_bytes, u32::MAX);
    }

    #[test]
    fn declared_content_length_ignores_invalid_values() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            CONTENT_LENGTH,
            reqwest::header::HeaderValue::from_static("not-a-number"),
        );

        assert_eq!(declared_content_length(&headers).unwrap(), None);
    }

    #[test]
    fn filename_from_content_disposition_prefers_rfc5987_filename_star() {
        let raw = "attachment; filename=\"fallback.txt\"; filename*=UTF-8''from%20star.txt";

        assert_eq!(
            filename_from_content_disposition(raw).as_deref(),
            Some("from star.txt")
        );
    }

    #[test]
    fn normalize_request_trims_filename_and_sha256() {
        let mut params = request(" https://example.com/file.bin ");
        params.filename = Some(" file.bin ".to_string());
        params.target_folder_id = Some(42);
        params.expected_sha256 =
            Some(" ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789 ".to_string());

        let normalized = normalize_offline_download_request(params).unwrap();

        assert_eq!(normalized.url.as_str(), "https://example.com/file.bin");
        assert_eq!(normalized.filename.as_deref(), Some("file.bin"));
        assert_eq!(normalized.target_folder_id, Some(42));
        assert_eq!(
            normalized.expected_sha256.as_deref(),
            Some("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
        );
    }

    #[test]
    fn normalize_request_rejects_invalid_sha256() {
        let mut params = request("https://example.com/file.bin");
        params.expected_sha256 = Some("not-a-sha".to_string());

        assert!(normalize_offline_download_request(params).is_err());
    }

    #[test]
    fn resolve_filename_prefers_requested_then_response_then_url_path() {
        let url = Url::parse("https://example.com/downloads/from-url%20name.txt?token=1").unwrap();

        assert_eq!(
            resolve_offline_download_filename(Some("requested.txt"), Some("response.txt"), &url)
                .unwrap(),
            "requested.txt"
        );
        assert_eq!(
            resolve_offline_download_filename(None, Some("response.txt"), &url).unwrap(),
            "response.txt"
        );
        assert_eq!(
            resolve_offline_download_filename(None, None, &url).unwrap(),
            "from-url name.txt"
        );
    }

    #[test]
    fn resolve_filename_falls_back_when_url_segment_is_not_valid_name() {
        let url = Url::parse("https://example.com/downloads/CON").unwrap();

        assert_eq!(
            resolve_offline_download_filename(None, None, &url).unwrap(),
            "download"
        );
    }

    #[test]
    fn aria2_options_use_safe_whitelist_and_per_download_limit() {
        let engine = Aria2OfflineDownloadEngine {
            max_bytes: 1024,
            download_timeout: StdDuration::from_secs(60),
            client: Aria2RpcClient::new(
                "http://127.0.0.1:6800/jsonrpc",
                Some("secret".to_string()),
                StdDuration::from_secs(1),
            )
            .unwrap(),
            split: 4,
            max_connection_per_server: 2,
            lowest_speed_limit_bytes_per_sec: Some(128),
        };
        let request = OfflineDownloadStartRequest {
            url: Url::parse("https://example.com/file.bin").unwrap(),
            temp_path: PathBuf::from("/tmp/asterdrive-task/source"),
            expected_sha256: None,
            max_bytes_per_sec: Some(1024),
            runtime_json: None,
        };

        let options = serde_json::to_value(engine.options(&request)).unwrap();

        assert_eq!(
            options.get("dir"),
            Some(&Value::String("/tmp/asterdrive-task".to_string()))
        );
        assert_eq!(
            options.get("out"),
            Some(&Value::String("source".to_string()))
        );
        assert_eq!(options.get("split"), Some(&Value::String("4".to_string())));
        assert_eq!(
            options.get("max-connection-per-server"),
            Some(&Value::String("2".to_string()))
        );
        assert_eq!(
            options.get("lowest-speed-limit"),
            Some(&Value::String("128".to_string()))
        );
        assert_eq!(
            options.get("max-download-limit"),
            Some(&Value::String("1024".to_string()))
        );
        assert!(
            options.get("max-overall-download-limit").is_none(),
            "task-level settings must not mutate aria2 global bandwidth limits"
        );
    }

    #[test]
    fn aria2_rpc_params_put_secret_token_first() {
        let client = Aria2RpcClient::new(
            "http://127.0.0.1:6800/jsonrpc",
            Some("rpc-secret".to_string()),
            StdDuration::from_secs(1),
        )
        .unwrap();

        assert_eq!(
            serde_json::to_value(client.params(Aria2RpcParam::String("gid-1".to_string()), None))
                .unwrap(),
            json!(["token:rpc-secret", "gid-1"])
        );
    }

    #[test]
    fn aria2_add_uri_params_match_json_rpc_shape() {
        let client = Aria2RpcClient::new(
            "http://127.0.0.1:6800/jsonrpc",
            Some("rpc-secret".to_string()),
            StdDuration::from_secs(1),
        )
        .unwrap();
        let options = Aria2AddUriOptions {
            dir: "/tmp/task".to_string(),
            out: "source".to_string(),
            allow_overwrite: "true".to_string(),
            auto_file_renaming: "false".to_string(),
            follow_torrent: "false".to_string(),
            follow_metalink: "false".to_string(),
            max_redirect: "0".to_string(),
            user_agent: "AsterDrive-test".to_string(),
            split: "1".to_string(),
            max_connection_per_server: "1".to_string(),
            lowest_speed_limit: None,
            max_download_limit: Some("1024".to_string()),
        };

        assert_eq!(
            serde_json::to_value(client.params(
                Aria2RpcParam::Uris(vec!["https://example.com/".to_string()]),
                Some(Aria2RpcParam::AddUriOptions(options))
            ))
            .unwrap(),
            json!([
                "token:rpc-secret",
                ["https://example.com/"],
                {
                    "dir": "/tmp/task",
                    "out": "source",
                    "allow-overwrite": "true",
                    "auto-file-renaming": "false",
                    "follow-torrent": "false",
                    "follow-metalink": "false",
                    "max-redirect": "0",
                    "user-agent": "AsterDrive-test",
                    "split": "1",
                    "max-connection-per-server": "1",
                    "max-download-limit": "1024"
                }
            ])
        );
    }

    #[test]
    fn aria2_rpc_params_omit_secret_when_absent() {
        let client = Aria2RpcClient::new(
            "http://127.0.0.1:6800/jsonrpc",
            None,
            StdDuration::from_secs(1),
        )
        .unwrap();

        assert_eq!(
            serde_json::to_value(client.params(Aria2RpcParam::String("gid-1".to_string()), None))
                .unwrap(),
            json!(["gid-1"])
        );
    }

    #[test]
    fn aria2_options_omit_optional_limits_when_unset() {
        let engine = Aria2OfflineDownloadEngine {
            max_bytes: 1024,
            download_timeout: StdDuration::from_secs(60),
            client: Aria2RpcClient::new(
                "http://127.0.0.1:6800/jsonrpc",
                None,
                StdDuration::from_secs(1),
            )
            .unwrap(),
            split: 1,
            max_connection_per_server: 1,
            lowest_speed_limit_bytes_per_sec: None,
        };
        let request = OfflineDownloadStartRequest {
            url: Url::parse("https://example.com/file.bin").unwrap(),
            temp_path: PathBuf::from("/tmp/asterdrive-task/source"),
            expected_sha256: None,
            max_bytes_per_sec: None,
            runtime_json: None,
        };

        let options = engine.options(&request);

        let options = serde_json::to_value(options).unwrap();
        assert!(options.get("lowest-speed-limit").is_none());
        assert!(options.get("max-download-limit").is_none());
        assert!(options.get("max-overall-download-limit").is_none());
    }

    #[test]
    fn aria2_rpc_call_error_preserves_structured_code_until_conversion() {
        let missing = Aria2RpcCallError::Rpc {
            method: "aria2.forceRemove",
            code: 1,
            message: "GID not found".to_string(),
        };
        assert!(matches!(missing, Aria2RpcCallError::Rpc { code: 1, .. }));

        let other = Aria2RpcCallError::Rpc {
            method: "aria2.forceRemove",
            code: 10,
            message: "other failure".to_string(),
        };
        assert!(!matches!(other, Aria2RpcCallError::Rpc { code: 1, .. }));

        let converted = other.into_aster_error();
        assert!(matches!(
            converted,
            AsterError::StorageDriverError(message)
                if message.contains("aria2.forceRemove failed with code 10: other failure")
        ));
    }

    #[test]
    fn aria2_download_status_deserializes_known_and_unknown_values() {
        #[derive(Deserialize)]
        struct Wrapper {
            status: Aria2DownloadStatus,
        }

        assert!(matches!(
            serde_json::from_str::<Wrapper>(r#"{"status":"active"}"#)
                .unwrap()
                .status,
            Aria2DownloadStatus::Active
        ));
        assert!(matches!(
            serde_json::from_str::<Wrapper>(r#"{"status":"future"}"#)
                .unwrap()
                .status,
            Aria2DownloadStatus::Unknown(value) if value == "future"
        ));
    }

    #[test]
    fn parse_aria2_length_handles_empty_and_rejects_invalid_or_oversized_values() {
        assert_eq!(parse_aria2_length("", "aria2 totalLength").unwrap(), 0);
        assert_eq!(parse_aria2_length("0", "aria2 totalLength").unwrap(), 0);
        assert_eq!(parse_aria2_length("42", "aria2 totalLength").unwrap(), 42);
        assert!(parse_aria2_length("-1", "aria2 totalLength").is_err());
        assert!(parse_aria2_length("not-a-number", "aria2 totalLength").is_err());
        assert!(parse_aria2_length(&u64::MAX.to_string(), "aria2 totalLength").is_err());
    }

    #[test]
    fn offline_download_runtime_state_round_trips_aria2_gid() {
        let runtime = OfflineDownloadRuntimeState {
            aria2: Some(Aria2TaskRuntime {
                gid: "abc123".to_string(),
                processing_token: 7,
            }),
        };
        let raw = serde_json::to_string(&runtime).unwrap();
        let decoded = decode_offline_download_runtime_state(Some(&raw));

        let aria2 = decoded.aria2.unwrap();
        assert_eq!(aria2.gid, "abc123");
        assert_eq!(aria2.processing_token, 7);
    }

    #[test]
    fn offline_download_runtime_state_ignores_missing_blank_and_invalid_json() {
        assert!(decode_offline_download_runtime_state(None).aria2.is_none());
        assert!(
            decode_offline_download_runtime_state(Some("  "))
                .aria2
                .is_none()
        );
        assert!(
            decode_offline_download_runtime_state(Some("{not-json"))
                .aria2
                .is_none()
        );
    }
}
