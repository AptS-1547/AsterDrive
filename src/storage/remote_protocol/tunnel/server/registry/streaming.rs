use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::{mpsc, oneshot};
use tokio_util::io::StreamReader;

use crate::entities::managed_follower;
use crate::errors::{AsterError, Result};
use crate::storage::error::{StorageErrorKind, storage_driver_error};

use super::broker::RemoteTunnelStreamHttpResponse;
use super::headers::request_headers;
use super::{
    REMOTE_TUNNEL_CONNECT_WAIT_TIMEOUT, REMOTE_TUNNEL_REQUEST_TIMEOUT,
    REMOTE_TUNNEL_STREAM_CHANNEL_CAPACITY, RemoteTunnelRegistry, reverse_tunnel_offline_error,
};
use super::super::response::header_pairs_to_map;
use super::super::{
    REMOTE_TUNNEL_STREAM_CHUNK_SIZE, RemoteTunnelStreamFrame, RemoteTunnelStreamFrameKind,
};

struct PinnedAsyncRead {
    inner: Pin<Box<dyn AsyncRead + Send>>,
    registry: Arc<RemoteTunnelRegistry>,
    request_id: String,
}

impl AsyncRead for PinnedAsyncRead {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        self.inner.as_mut().poll_read(cx, buf)
    }
}

impl Drop for PinnedAsyncRead {
    fn drop(&mut self) {
        self.registry.stream_pending.remove(&self.request_id);
    }
}

#[derive(Debug)]
pub(super) struct StreamingTunnelLane {
    lane_id: String,
    pub(super) remote_node_id: i64,
    request_tx: mpsc::Sender<RemoteTunnelStreamFrame>,
    busy: AtomicBool,
}

struct PendingStreamStart {
    status: StatusCode,
    headers: HeaderMap,
}

pub(super) struct PendingStreamResponse {
    pub(super) remote_node_id: i64,
    lane_id: String,
    start_tx: parking_lot::Mutex<Option<oneshot::Sender<Result<PendingStreamStart>>>>,
    body_tx: mpsc::Sender<PendingStreamBodyFrame>,
    _lane_lease: StreamingLaneLease,
}

enum PendingStreamBodyFrame {
    Chunk(std::io::Result<Bytes>),
    End,
}

struct StreamingLaneLease {
    registry: Arc<RemoteTunnelRegistry>,
    lane: Arc<StreamingTunnelLane>,
}

impl Drop for StreamingLaneLease {
    fn drop(&mut self) {
        self.lane.busy.store(false, Ordering::Release);
        self.registry.connection_notify.notify_waiters();
    }
}

pub(crate) struct RemoteTunnelStreamRegistrationGuard {
    registry: Arc<RemoteTunnelRegistry>,
    access_key: String,
    remote_node_id: i64,
    lane_id: String,
}

impl Drop for RemoteTunnelStreamRegistrationGuard {
    fn drop(&mut self) {
        self.registry.unregister_stream_lane_if_same(
            &self.access_key,
            self.remote_node_id,
            &self.lane_id,
        );
        self.registry
            .fail_stream_requests_for_lane(&self.lane_id, "reverse tunnel streaming lane closed");
    }
}

impl RemoteTunnelRegistry {
    pub(crate) fn register_stream_lane(
        self: &Arc<Self>,
        remote_node: &managed_follower::Model,
    ) -> (
        String,
        mpsc::Receiver<RemoteTunnelStreamFrame>,
        RemoteTunnelStreamRegistrationGuard,
    ) {
        let (request_tx, request_rx) = mpsc::channel(REMOTE_TUNNEL_STREAM_CHANNEL_CAPACITY);
        let lane_id = crate::utils::id::new_uuid();
        let lane = Arc::new(StreamingTunnelLane {
            lane_id: lane_id.clone(),
            remote_node_id: remote_node.id,
            request_tx,
            busy: AtomicBool::new(false),
        });
        self.stream_lanes
            .entry(remote_node.access_key.clone())
            .or_default()
            .push(lane);
        self.connection_notify.notify_waiters();
        let guard = RemoteTunnelStreamRegistrationGuard {
            registry: self.clone(),
            access_key: remote_node.access_key.clone(),
            remote_node_id: remote_node.id,
            lane_id: lane_id.clone(),
        };
        (lane_id, request_rx, guard)
    }

    fn unregister_stream_lane_if_same(&self, access_key: &str, remote_node_id: i64, lane_id: &str) {
        let mut should_remove_entry = false;
        if let Some(mut lanes) = self.stream_lanes.get_mut(access_key) {
            lanes
                .retain(|lane| !(lane.remote_node_id == remote_node_id && lane.lane_id == lane_id));
            should_remove_entry = lanes.is_empty();
        }
        if should_remove_entry {
            self.stream_lanes.remove(access_key);
        }
    }

    pub async fn send_stream(
        self: &Arc<Self>,
        remote_node: &managed_follower::Model,
        method: Method,
        path_and_query: String,
        content_length: Option<u64>,
        extra_headers: Vec<(String, String)>,
        mut body: Box<dyn AsyncRead + Unpin + Send>,
    ) -> Result<RemoteTunnelStreamHttpResponse> {
        let lane = match self.stream_lane(remote_node).await {
            Ok(lane) => lane,
            Err(error) => {
                self.record_error(remote_node.id, error.message());
                return Err(error);
            }
        };
        let lane_lease = StreamingLaneLease {
            registry: self.clone(),
            lane: lane.clone(),
        };

        let request_id = crate::utils::id::new_uuid();
        let (start_tx, start_rx) = oneshot::channel();
        let (body_tx, body_rx) = mpsc::channel(REMOTE_TUNNEL_STREAM_CHANNEL_CAPACITY);
        self.stream_pending.insert(
            request_id.clone(),
            PendingStreamResponse {
                remote_node_id: remote_node.id,
                lane_id: lane.lane_id.clone(),
                start_tx: parking_lot::Mutex::new(Some(start_tx)),
                body_tx,
                _lane_lease: lane_lease,
            },
        );

        let headers = request_headers(remote_node, &method, &path_and_query, content_length)
            .chain(extra_headers)
            .collect();
        let request_start = RemoteTunnelStreamFrame {
            kind: RemoteTunnelStreamFrameKind::RequestStart,
            request_id: request_id.clone(),
            method: Some(method.as_str().to_string()),
            path_and_query: Some(path_and_query),
            headers,
            content_length,
            status: None,
            message: None,
            body: Bytes::new(),
        };
        if let Err(error) = lane.request_tx.send(request_start).await {
            self.stream_pending.remove(&request_id);
            let error = storage_driver_error(
                StorageErrorKind::Transient,
                format!("reverse tunnel streaming lane closed before request start: {error}"),
            );
            self.record_error(remote_node.id, error.message());
            return Err(error);
        }

        let request_sender = lane.request_tx.clone();
        let request_id_for_body = request_id.clone();
        tokio::spawn(async move {
            let result =
                send_stream_request_body(&request_sender, &request_id_for_body, &mut body).await;
            if let Err(error) = result {
                let _ = request_sender
                    .send(RemoteTunnelStreamFrame::error(
                        request_id_for_body,
                        error.message().to_string(),
                    ))
                    .await;
            }
        });

        let start = match tokio::time::timeout(REMOTE_TUNNEL_REQUEST_TIMEOUT, start_rx).await {
            Ok(Ok(Ok(start))) => start,
            Ok(Ok(Err(error))) => {
                self.stream_pending.remove(&request_id);
                self.record_error(remote_node.id, error.message());
                return Err(error);
            }
            Ok(Err(error)) => {
                self.stream_pending.remove(&request_id);
                let error = storage_driver_error(
                    StorageErrorKind::Transient,
                    format!("reverse tunnel streaming response channel closed: {error}"),
                );
                self.record_error(remote_node.id, error.message());
                return Err(error);
            }
            Err(_) => {
                self.stream_pending.remove(&request_id);
                let error = storage_driver_error(
                    StorageErrorKind::Transient,
                    "reverse tunnel streaming request timed out waiting for follower response",
                );
                self.record_error(remote_node.id, error.message());
                return Err(error);
            }
        };

        if start.status.is_success() {
            self.clear_error(remote_node.id);
        } else {
            self.record_error(
                remote_node.id,
                format!("reverse tunnel returned HTTP {}", start.status),
            );
        }

        let request_id_for_stream = request_id.clone();
        let registry = self.clone();
        let stream = async_stream::stream! {
            let mut body_rx = body_rx;
            while let Some(frame) = body_rx.recv().await {
                match frame {
                    PendingStreamBodyFrame::Chunk(chunk) => yield chunk,
                    PendingStreamBodyFrame::End => break,
                }
            }
        };
        let reader = StreamReader::new(stream);
        Ok(RemoteTunnelStreamHttpResponse {
            status: start.status,
            headers: start.headers,
            body: Box::new(PinnedAsyncRead {
                inner: Box::pin(reader),
                registry,
                request_id: request_id_for_stream,
            }),
        })
    }

    async fn stream_lane(
        self: &Arc<Self>,
        remote_node: &managed_follower::Model,
    ) -> Result<Arc<StreamingTunnelLane>> {
        tokio::time::timeout(REMOTE_TUNNEL_CONNECT_WAIT_TIMEOUT, async {
            loop {
                let notified = self.connection_notify.notified();
                if let Some(lanes) = self.stream_lanes.get(&remote_node.access_key) {
                    for lane in lanes.iter() {
                        if lane.remote_node_id == remote_node.id
                            && lane
                                .busy
                                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                                .is_ok()
                        {
                            return lane.clone();
                        }
                    }
                }
                notified.await;
            }
        })
        .await
        .map_err(|_| reverse_tunnel_offline_error(remote_node.id))
    }

    pub(crate) async fn complete_stream_frame(
        &self,
        remote_node: &managed_follower::Model,
        lane_id: &str,
        frame: RemoteTunnelStreamFrame,
    ) -> Result<()> {
        let request_id = frame.request_id.clone();
        let Some(pending) = self.stream_pending.get(&request_id) else {
            return Err(AsterError::validation_error(
                "reverse tunnel streaming request is no longer pending",
            ));
        };
        if pending.remote_node_id != remote_node.id {
            return Err(AsterError::auth_invalid_credentials(
                "reverse tunnel streaming completion does not belong to this remote node",
            ));
        }
        if pending.lane_id != lane_id {
            return Err(AsterError::auth_invalid_credentials(
                "reverse tunnel streaming completion does not belong to this lane",
            ));
        }

        match frame.kind {
            RemoteTunnelStreamFrameKind::ResponseStart => {
                let status = frame.status.ok_or_else(|| {
                    AsterError::validation_error("stream response_start missing status")
                })?;
                let start = PendingStreamStart {
                    status: StatusCode::from_u16(status).map_err(|error| {
                        storage_driver_error(
                            StorageErrorKind::Misconfigured,
                            format!("reverse tunnel stream returned invalid HTTP status: {error}"),
                        )
                    })?,
                    headers: header_pairs_to_map(frame.headers)?,
                };
                let sender = pending.start_tx.lock().take();
                drop(pending);
                let Some(sender) = sender else {
                    return Err(AsterError::validation_error(
                        "reverse tunnel streaming response already started",
                    ));
                };
                sender.send(Ok(start)).map_err(|_| {
                    AsterError::validation_error(
                        "reverse tunnel streaming response receiver closed before start",
                    )
                })
            }
            RemoteTunnelStreamFrameKind::ResponseBody => {
                let body_tx = pending.body_tx.clone();
                drop(pending);
                body_tx
                    .send(PendingStreamBodyFrame::Chunk(Ok(frame.body)))
                    .await
                    .map_err(|_| {
                        AsterError::validation_error(
                            "reverse tunnel streaming response receiver closed before body",
                        )
                    })
            }
            RemoteTunnelStreamFrameKind::ResponseEnd => {
                let body_tx = pending.body_tx.clone();
                drop(pending);
                body_tx
                    .send(PendingStreamBodyFrame::End)
                    .await
                    .map_err(|_| {
                        AsterError::validation_error(
                            "reverse tunnel streaming response receiver closed before end",
                        )
                    })
            }
            RemoteTunnelStreamFrameKind::Error => {
                let message = frame
                    .message
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "reverse tunnel streaming lane reported error".to_string());
                let sender = pending.start_tx.lock().take();
                let body_tx = pending.body_tx.clone();
                drop(pending);
                let error = storage_driver_error(StorageErrorKind::Transient, message);
                if let Some(sender) = sender {
                    let _ = sender.send(Err(error.clone()));
                } else {
                    let _ = body_tx
                        .send(PendingStreamBodyFrame::Chunk(Err(std::io::Error::other(
                            error.message().to_string(),
                        ))))
                        .await;
                }
                Err(error)
            }
            _ => Err(AsterError::validation_error(
                "invalid reverse tunnel streaming response frame",
            )),
        }
    }

    fn fail_stream_requests_for_lane(&self, lane_id: &str, message: &str) {
        let request_ids = self
            .stream_pending
            .iter()
            .filter(|entry| entry.lane_id == lane_id)
            .map(|entry| entry.key().clone())
            .collect::<Vec<_>>();
        for request_id in request_ids {
            if let Some((_, pending)) = self.stream_pending.remove(&request_id) {
                let error = storage_driver_error(StorageErrorKind::Transient, message);
                if let Some(sender) = pending.start_tx.lock().take() {
                    let _ = sender.send(Err(error));
                } else {
                    let _ = pending.body_tx.try_send(PendingStreamBodyFrame::Chunk(Err(
                        std::io::Error::other(message.to_owned()),
                    )));
                }
            }
        }
    }

    pub fn has_stream_lane(&self, remote_node: &managed_follower::Model) -> bool {
        self.stream_lanes
            .get(&remote_node.access_key)
            .map(|lanes| {
                lanes
                    .iter()
                    .any(|lane| lane.remote_node_id == remote_node.id)
            })
            .unwrap_or(false)
    }

    #[cfg(test)]
    pub(crate) fn has_pending_stream_response(&self, request_id: &str) -> bool {
        self.stream_pending.contains_key(request_id)
    }
}

async fn send_stream_request_body(
    request_tx: &mpsc::Sender<RemoteTunnelStreamFrame>,
    request_id: &str,
    body: &mut (dyn AsyncRead + Unpin + Send),
) -> Result<()> {
    let mut buffer = vec![0u8; REMOTE_TUNNEL_STREAM_CHUNK_SIZE];
    loop {
        let read = body.read(&mut buffer).await.map_err(|error| {
            storage_driver_error(
                StorageErrorKind::Transient,
                format!("read reverse tunnel streaming request body: {error}"),
            )
        })?;
        if read == 0 {
            break;
        }
        request_tx
            .send(RemoteTunnelStreamFrame {
                kind: RemoteTunnelStreamFrameKind::RequestBody,
                request_id: request_id.to_string(),
                method: None,
                path_and_query: None,
                headers: Vec::new(),
                content_length: None,
                status: None,
                message: None,
                body: Bytes::copy_from_slice(&buffer[..read]),
            })
            .await
            .map_err(|error| {
                storage_driver_error(
                    StorageErrorKind::Transient,
                    format!("reverse tunnel streaming lane closed while sending body: {error}"),
                )
            })?;
    }

    request_tx
        .send(RemoteTunnelStreamFrame {
            kind: RemoteTunnelStreamFrameKind::RequestEnd,
            request_id: request_id.to_string(),
            method: None,
            path_and_query: None,
            headers: Vec::new(),
            content_length: None,
            status: None,
            message: None,
            body: Bytes::new(),
        })
        .await
        .map_err(|error| {
            storage_driver_error(
                StorageErrorKind::Transient,
                format!("reverse tunnel streaming lane closed before request end: {error}"),
            )
        })
}
