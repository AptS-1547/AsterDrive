//! 集成测试：`errors`。

use actix_web::{ResponseError, body::to_bytes, http::StatusCode};
use aster_drive::errors::AsterError;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::{
    layer::{Context, Layer},
    prelude::*,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedEvent {
    level: Level,
    message: Option<String>,
}

#[derive(Clone, Default)]
struct EventRecorder {
    events: Arc<Mutex<Vec<RecordedEvent>>>,
}

#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}").trim_matches('"').to_string());
        }
    }
}

impl<S> Layer<S> for EventRecorder
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        self.events.lock().unwrap().push(RecordedEvent {
            level: *event.metadata().level(),
            message: visitor.message,
        });
    }
}

fn capture_events(f: impl FnOnce()) -> Vec<RecordedEvent> {
    let recorder = EventRecorder::default();
    let subscriber = tracing_subscriber::registry().with(recorder.clone());

    tracing::subscriber::with_default(subscriber, f);

    recorder.events.lock().unwrap().clone()
}

async fn response_body_json(resp: actix_web::HttpResponse) -> Value {
    let body = to_bytes(resp.into_body()).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[test]
fn storage_quota_exceeded_logs_warn_for_507() {
    let err = AsterError::storage_quota_exceeded("quota 1024, used 1000, need 100");

    let events = capture_events(|| {
        let resp = err.error_response();
        assert_eq!(resp.status(), StatusCode::INSUFFICIENT_STORAGE);
    });

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].level, Level::WARN);
    assert_eq!(events[0].message.as_deref(), Some("request error"));
}

#[test]
fn internal_error_logs_error() {
    let err = AsterError::internal_error("db pool poisoned");

    let events = capture_events(|| {
        let resp = err.error_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    });

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].level, Level::ERROR);
    assert_eq!(events[0].message.as_deref(), Some("server error"));
}

#[test]
fn unauthorized_error_skips_logging() {
    let err = AsterError::auth_token_invalid("invalid token");

    let events = capture_events(|| {
        let resp = err.error_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    });

    assert!(events.is_empty());
}

#[test]
fn validation_error_logs_warn() {
    let err = AsterError::validation_error("file name is invalid");

    let events = capture_events(|| {
        let resp = err.error_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    });

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].level, Level::WARN);
    assert_eq!(events[0].message.as_deref(), Some("request error"));
}

#[actix_web::test]
async fn internal_error_redacts_response_message() {
    let err = AsterError::internal_error("db pool poisoned");

    let resp = err.error_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = response_body_json(resp).await;
    assert_eq!(body["msg"], "Internal Server Error");
}

#[actix_web::test]
async fn storage_driver_error_redacts_response_message() {
    let err = AsterError::storage_driver_error("read file: /tmp/private/secret.txt");

    let resp = err.error_response();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let body = response_body_json(resp).await;
    assert_eq!(body["msg"], "Storage Driver Error");
}

#[actix_web::test]
async fn storage_quota_exceeded_keeps_response_message() {
    let err = AsterError::storage_quota_exceeded("quota 1024, used 1000, need 100");

    let resp = err.error_response();
    assert_eq!(resp.status(), StatusCode::INSUFFICIENT_STORAGE);

    let body = response_body_json(resp).await;
    assert_eq!(body["msg"], "quota 1024, used 1000, need 100");
}
