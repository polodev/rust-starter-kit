//! The database mail queue (Laravel's `database` queue driver, for mail). `dispatch` stores a mail in `mail_queue`
//! and returns at once; a worker (`mail:work`, or the embedded one in development) sends it, and tries again later
//! when sending fails. `mail_log` keeps the history: `queued` while waiting, then `sent` or `failed`.

use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{Duration as ChronoDuration, Utc};
use loco_rs::{app::AppContext, Error, Result};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};

use crate::{
    config::{Delivery, MailConfig},
    entities::{mail_log, mail_queue},
    message::{Attachment, Mail},
    service, transport,
};

/// A mail is tried this many times in all before it is marked `failed`.
pub const MAX_ATTEMPTS: i32 = 3;
/// Waiting time before the next try, in minutes, after the first and the second failure.
const RETRY_AFTER_MINUTES: [i64; 2] = [1, 5];
/// A mail held by a worker that never reported back (it crashed) is taken over after this long.
const RESERVATION_MINUTES: i64 = 5;
const POLL_EVERY: Duration = Duration::from_secs(2);
const BATCH: u64 = 20;

#[derive(Serialize, Deserialize)]
struct QueuedAttachment {
    filename: String,
    content_type: String,
    base64: String,
}

/// The stored form of a [`Mail`].
#[derive(Serialize, Deserialize)]
struct QueuedMail {
    to: Vec<String>,
    cc: Vec<String>,
    bcc: Vec<String>,
    reply_to: Option<String>,
    subject: String,
    text: Option<String>,
    html: Option<String>,
    attachments: Vec<QueuedAttachment>,
}

impl QueuedMail {
    fn from_mail(mail: &Mail) -> Self {
        Self {
            to: mail.to.clone(),
            cc: mail.cc.clone(),
            bcc: mail.bcc.clone(),
            reply_to: mail.reply_to.clone(),
            subject: mail.subject.clone(),
            text: mail.text.clone(),
            html: mail.html.clone(),
            attachments: mail
                .attachments
                .iter()
                .map(|a| QueuedAttachment {
                    filename: a.filename.clone(),
                    content_type: a.content_type.clone(),
                    base64: STANDARD.encode(&a.bytes),
                })
                .collect(),
        }
    }

    fn into_mail(self) -> Result<Mail> {
        let mut attachments = Vec::new();
        for a in self.attachments {
            attachments.push(Attachment {
                filename: a.filename,
                content_type: a.content_type,
                bytes: STANDARD
                    .decode(a.base64)
                    .map_err(|error| Error::string(&error.to_string()))?,
            });
        }
        Ok(Mail {
            to: self.to,
            cc: self.cc,
            bcc: self.bcc,
            reply_to: self.reply_to,
            subject: self.subject,
            text: self.text,
            html: self.html,
            attachments,
        })
    }
}

/// Sends the mail now or queues it, as `settings.mail.delivery` says. Use this from request handlers.
pub async fn dispatch(ctx: &AppContext, mail: Mail) -> Result<mail_log::Model> {
    let config = MailConfig::load(ctx)?;
    match config.delivery {
        Delivery::Now => service::send_with(&ctx.db, &config, mail).await,
        Delivery::Queue => enqueue(&ctx.db, &config, mail).await,
    }
}

/// Stores the mail in `mail_queue` and writes its `queued` row in `mail_log`. A mail that cannot be built (no
/// recipient, bad address) is refused here, before anything is stored.
pub async fn enqueue<C: ConnectionTrait>(
    db: &C,
    config: &MailConfig,
    mail: Mail,
) -> Result<mail_log::Model> {
    mail.build(config)?;
    let payload = serde_json::to_string(&QueuedMail::from_mail(&mail))
        .map_err(|error| Error::string(&error.to_string()))?;
    let row = service::record_queued(db, config, &mail).await?;
    let now = Utc::now().fixed_offset();
    mail_queue::ActiveModel {
        mail_log_id: Set(row.id),
        payload: Set(payload),
        attempts: Set(0),
        available_at: Set(now),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(row)
}

/// Sends what is due, at most [`BATCH`] mails; returns how many were handled (sent or failed for good).
pub async fn work_once<C: ConnectionTrait>(db: &C, config: &MailConfig) -> Result<usize> {
    let now = Utc::now().fixed_offset();
    let stale = now - ChronoDuration::minutes(RESERVATION_MINUTES);
    let due = mail_queue::Entity::find()
        .filter(mail_queue::Column::AvailableAt.lte(now))
        .filter(
            Condition::any()
                .add(mail_queue::Column::ReservedAt.is_null())
                .add(mail_queue::Column::ReservedAt.lt(stale)),
        )
        .order_by_asc(mail_queue::Column::Id)
        .limit(BATCH)
        .all(db)
        .await?;

    let mut handled = 0;
    for entry in due {
        // Several workers may run; only the one whose update changes the row owns the mail.
        let claimed = mail_queue::Entity::update_many()
            .col_expr(mail_queue::Column::ReservedAt, Expr::value(now))
            .filter(mail_queue::Column::Id.eq(entry.id))
            .filter(
                Condition::any()
                    .add(mail_queue::Column::ReservedAt.is_null())
                    .add(mail_queue::Column::ReservedAt.lt(stale)),
            )
            .exec(db)
            .await?;
        if claimed.rows_affected == 1 && attempt(db, config, entry).await? {
            handled += 1;
        }
    }
    Ok(handled)
}

/// One try. Returns true when the mail is finished (sent, or out of attempts).
async fn attempt<C: ConnectionTrait>(
    db: &C,
    config: &MailConfig,
    entry: mail_queue::Model,
) -> Result<bool> {
    let outcome = async {
        let queued: QueuedMail = serde_json::from_str(&entry.payload)
            .map_err(|error| Error::string(&error.to_string()))?;
        let message = queued.into_mail()?.build(config)?;
        transport::deliver(config, message).await
    }
    .await;

    let finished = Utc::now().fixed_offset();
    let attempts = entry.attempts + 1;
    let log = mail_log::Entity::find_by_id(entry.mail_log_id)
        .one(db)
        .await?;
    let mut log: Option<mail_log::ActiveModel> = log.map(Into::into);
    if let Some(log) = log.as_mut() {
        log.attempts = Set(attempts);
        log.updated_at = Set(Some(finished));
    }

    match outcome {
        Ok(delivered) => {
            if let Some(mut log) = log {
                log.status = Set(mail_log::SENT.to_string());
                log.error = Set(None);
                log.provider_id = Set(delivered
                    .provider_id
                    .map(|id| id.chars().take(255).collect()));
                log.sent_at = Set(Some(finished));
                log.update(db).await?;
            }
            mail_queue::Entity::delete_by_id(entry.id).exec(db).await?;
            Ok(true)
        }
        Err(error) => {
            let reason: String = error.to_string().chars().take(2000).collect();
            let out_of_attempts = attempts >= MAX_ATTEMPTS;
            if let Some(mut log) = log {
                if out_of_attempts {
                    log.status = Set(mail_log::FAILED.to_string());
                }
                log.error = Set(Some(reason));
                log.update(db).await?;
            }
            if out_of_attempts {
                mail_queue::Entity::delete_by_id(entry.id).exec(db).await?;
                return Ok(true);
            }
            let wait = RETRY_AFTER_MINUTES
                .get((attempts - 1) as usize)
                .copied()
                .unwrap_or(5);
            let mut retry: mail_queue::ActiveModel = entry.into();
            retry.attempts = Set(attempts);
            retry.reserved_at = Set(None);
            retry.available_at = Set(finished + ChronoDuration::minutes(wait));
            retry.updated_at = Set(Some(finished));
            retry.update(db).await?;
            Ok(false)
        }
    }
}

/// The worker: looks for due mail every couple of seconds, forever. Run by `mail:work` (under Supervisor in
/// production) or started inside the web server in development (`embedded_worker`).
pub async fn run_worker(ctx: &AppContext) -> Result<()> {
    let config = MailConfig::load(ctx)?;
    loop {
        if let Err(error) = work_once(&ctx.db, &config).await {
            tracing::warn!(%error, "mail worker pass failed");
        }
        tokio::time::sleep(POLL_EVERY).await;
    }
}

/// Starts the worker in the background of this process when `settings.mail.embedded_worker` is on and mail is
/// queued. Safe next to a separate `mail:work`: a mail is claimed by one worker only.
pub fn spawn_embedded_worker(ctx: &AppContext) {
    let enabled = MailConfig::load(ctx)
        .is_ok_and(|config| config.embedded_worker && config.delivery == Delivery::Queue);
    if !enabled {
        return;
    }
    let ctx = ctx.clone();
    tokio::spawn(async move {
        tracing::info!("mail queue worker started inside the web server");
        if let Err(error) = run_worker(&ctx).await {
            tracing::error!(%error, "mail queue worker stopped");
        }
    });
}
