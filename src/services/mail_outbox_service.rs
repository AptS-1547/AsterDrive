//! 服务模块：`mail_outbox_service`。

use std::sync::Arc;

use chrono::{Duration, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, Set};

use crate::config::RuntimeConfig;
use crate::db::repository::mail_outbox_repo;
use crate::entities::mail_outbox;
use crate::errors::Result;
use crate::runtime::AppState;
use crate::services::{
    mail_service,
    mail_service::MailSender,
    mail_template::{self, MailTemplatePayload},
};
use crate::types::MailOutboxStatus;

const MAIL_OUTBOX_BATCH_SIZE: u64 = 20;
const MAIL_OUTBOX_PROCESSING_STALE_SECS: i64 = 60;
const MAIL_OUTBOX_MAX_ATTEMPTS: i32 = 6;
const MAIL_OUTBOX_MAX_ERROR_LEN: usize = 1024;
const MAIL_OUTBOX_DRAIN_MAX_ROUNDS: usize = 32;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DispatchStats {
    pub claimed: usize,
    pub sent: usize,
    pub retried: usize,
    pub failed: usize,
}

impl DispatchStats {
    fn merge(&mut self, other: DispatchStats) {
        self.claimed += other.claimed;
        self.sent += other.sent;
        self.retried += other.retried;
        self.failed += other.failed;
    }
}

pub(crate) async fn enqueue<C: ConnectionTrait>(
    db: &C,
    to_address: &str,
    to_name: Option<&str>,
    payload: MailTemplatePayload,
) -> Result<mail_outbox::Model> {
    let now = Utc::now();
    mail_outbox_repo::create(
        db,
        mail_outbox::ActiveModel {
            template_code: Set(payload.template_code()),
            to_address: Set(to_address.to_string()),
            to_name: Set(to_name.map(str::to_string)),
            payload_json: Set(payload.to_stored()?),
            status: Set(MailOutboxStatus::Pending),
            attempt_count: Set(0),
            next_attempt_at: Set(now),
            processing_started_at: Set(None),
            sent_at: Set(None),
            last_error: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        },
    )
    .await
}

pub async fn dispatch_due(state: &AppState) -> Result<DispatchStats> {
    dispatch_due_with(&state.db, &state.runtime_config, &state.mail_sender).await
}

pub async fn dispatch_due_with(
    db: &DatabaseConnection,
    runtime_config: &Arc<RuntimeConfig>,
    mail_sender: &Arc<dyn MailSender>,
) -> Result<DispatchStats> {
    let now = Utc::now();
    let stale_before = now - Duration::seconds(MAIL_OUTBOX_PROCESSING_STALE_SECS);
    let due =
        mail_outbox_repo::list_claimable(db, now, stale_before, MAIL_OUTBOX_BATCH_SIZE).await?;
    let mut stats = DispatchStats::default();

    for row in due {
        let claimed_at = Utc::now();
        if !mail_outbox_repo::try_claim(db, row.id, claimed_at, stale_before).await? {
            continue;
        }

        stats.claimed += 1;
        let mut claimed_row = row;
        claimed_row.status = MailOutboxStatus::Processing;
        claimed_row.processing_started_at = Some(claimed_at);
        claimed_row.updated_at = claimed_at;

        match deliver_one(runtime_config, mail_sender, &claimed_row).await {
            Ok(()) => {
                if mail_outbox_repo::mark_sent(db, claimed_row.id, Utc::now()).await? {
                    stats.sent += 1;
                }
            }
            Err(error) => {
                let attempt_count = claimed_row.attempt_count + 1;
                let error_message = truncate_error(&error.to_string());
                if attempt_count >= MAIL_OUTBOX_MAX_ATTEMPTS {
                    if mail_outbox_repo::mark_failed(
                        db,
                        claimed_row.id,
                        attempt_count,
                        Utc::now(),
                        &error_message,
                    )
                    .await?
                    {
                        stats.failed += 1;
                    }
                    tracing::warn!(
                        mail_outbox_id = claimed_row.id,
                        template_code = %claimed_row.template_code.as_str(),
                        to = %claimed_row.to_address,
                        attempt_count,
                        error = %error_message,
                        "mail outbox delivery permanently failed"
                    );
                } else {
                    let retry_at = Utc::now() + Duration::seconds(retry_delay_secs(attempt_count));
                    if mail_outbox_repo::mark_retry(
                        db,
                        claimed_row.id,
                        attempt_count,
                        retry_at,
                        &error_message,
                    )
                    .await?
                    {
                        stats.retried += 1;
                    }
                    tracing::warn!(
                        mail_outbox_id = claimed_row.id,
                        template_code = %claimed_row.template_code.as_str(),
                        to = %claimed_row.to_address,
                        attempt_count,
                        retry_at = %retry_at,
                        error = %error_message,
                        "mail outbox delivery failed; scheduled retry"
                    );
                }
            }
        }
    }

    Ok(stats)
}

pub async fn drain(state: &AppState) -> Result<DispatchStats> {
    drain_with(&state.db, &state.runtime_config, &state.mail_sender).await
}

pub async fn drain_with(
    db: &DatabaseConnection,
    runtime_config: &Arc<RuntimeConfig>,
    mail_sender: &Arc<dyn MailSender>,
) -> Result<DispatchStats> {
    let mut total = DispatchStats::default();

    for _ in 0..MAIL_OUTBOX_DRAIN_MAX_ROUNDS {
        let stats = dispatch_due_with(db, runtime_config, mail_sender).await?;
        let claimed = stats.claimed;
        total.merge(stats);
        if claimed == 0 {
            break;
        }
    }

    Ok(total)
}

async fn deliver_one(
    runtime_config: &RuntimeConfig,
    mail_sender: &Arc<dyn MailSender>,
    row: &mail_outbox::Model,
) -> Result<()> {
    let rendered = mail_template::render(runtime_config, row.template_code, &row.payload_json)?;
    mail_service::send_rendered_with(
        runtime_config,
        mail_sender,
        mail_service::MailRecipient {
            address: row.to_address.clone(),
            display_name: row.to_name.clone(),
        },
        rendered,
    )
    .await
}

fn retry_delay_secs(attempt_count: i32) -> i64 {
    match attempt_count {
        1 => 5,
        2 => 15,
        3 => 60,
        4 => 300,
        5 => 900,
        _ => 1800,
    }
}

fn truncate_error(error: &str) -> String {
    error.chars().take(MAIL_OUTBOX_MAX_ERROR_LEN).collect()
}
