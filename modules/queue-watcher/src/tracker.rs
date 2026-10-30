//! Records the life of a background job in `queue_jobs`: `queued`, `running`, then `finished` or `failed`.
//! Wrap the body of a worker in [`track`]:
//!
//! ```ignore
//! async fn perform(&self, args: MyArgs) -> Result<()> {
//!     module_queue_watcher::track(&self.ctx, JobInfo::new("SendDigest").queue("mail"), async {
//!         // the real work; an `Err` is recorded as a failed job and returned unchanged
//!         Ok(())
//!     })
//!     .await
//! }
//! ```
//!
//! A problem writing the history never fails the job itself; it is only logged.

use std::future::Future;

use chrono::{Duration, Utc};
use loco_rs::{app::AppContext, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, PaginatorTrait,
    QueryFilter, Set,
};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::{failed_jobs, jobs, queue_jobs};

const MAX_TRACE_CHARS: usize = 60_000;

/// What to record about a job.
#[derive(Debug, Clone)]
pub struct JobInfo {
    pub name: String,
    pub class: Option<String>,
    pub description: Option<String>,
    pub queue: String,
    pub connection: String,
    pub max_tries: Option<i32>,
    pub timeout: Option<i32>,
    pub tags: Option<String>,
    pub payload: Option<Value>,
}

impl JobInfo {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            class: None,
            description: None,
            queue: "default".to_string(),
            connection: "background".to_string(),
            max_tries: None,
            timeout: None,
            tags: None,
            payload: None,
        }
    }

    pub fn queue(mut self, queue: &str) -> Self {
        self.queue = queue.to_string();
        self
    }

    pub fn class(mut self, class: &str) -> Self {
        self.class = Some(class.to_string());
        self
    }

    pub fn description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn payload(mut self, payload: Value) -> Self {
        self.payload = Some(payload);
        self
    }

    pub fn max_tries(mut self, tries: i32) -> Self {
        self.max_tries = Some(tries);
        self
    }

    pub fn tags(mut self, tags: &str) -> Self {
        self.tags = Some(tags.to_string());
        self
    }
}

async fn insert<C: ConnectionTrait>(
    db: &C,
    info: &JobInfo,
    status: &str,
    started: bool,
) -> Result<queue_jobs::Model, DbErr> {
    let now = Utc::now().fixed_offset();
    queue_jobs::ActiveModel {
        job_id: Set(Uuid::new_v4().to_string()),
        name: Set(Some(info.name.clone())),
        job_class: Set(info.class.clone()),
        job_description: Set(info.description.clone()),
        queue: Set(Some(info.queue.clone())),
        connection: Set(Some(info.connection.clone())),
        started_at: Set(started.then_some(now)),
        attempt: Set(i32::from(started)),
        max_tries: Set(info.max_tries),
        timeout: Set(info.timeout),
        tags: Set(info.tags.clone()),
        status: Set(status.to_string()),
        payload: Set(info.payload.as_ref().map(Value::to_string)),
        created_at: Set(Some(now)),
        updated_at: Set(Some(now)),
        ..Default::default()
    }
    .insert(db)
    .await
}

/// Records a job that has been put on a queue but has not started. Returns its `job_id`; pass it to
/// [`track_queued`] when the worker picks it up.
pub async fn queued(ctx: &AppContext, info: &JobInfo) -> Result<String> {
    Ok(insert(&ctx.db, info, queue_jobs::QUEUED, false)
        .await?
        .job_id)
}

async fn finish(ctx: &AppContext, id: u64, outcome: &Result<()>) {
    let Ok(Some(row)) = queue_jobs::Entity::find_by_id(id).one(&ctx.db).await else {
        return;
    };
    let now = Utc::now().fixed_offset();
    let mut active: queue_jobs::ActiveModel = row.into();
    match outcome {
        Ok(()) => {
            active.status = Set(queue_jobs::FINISHED.to_string());
            active.finished_at = Set(Some(now));
        }
        Err(error) => {
            active.status = Set(queue_jobs::FAILED.to_string());
            active.failed_at = Set(Some(now));
            active.exception_message = Set(Some(error.to_string()));
            let trace: String = format!("{error:?}").chars().take(MAX_TRACE_CHARS).collect();
            active.exception_trace = Set(Some(trace));
        }
    }
    active.updated_at = Set(Some(now));
    if let Err(error) = active.update(&ctx.db).await {
        tracing::warn!(%error, "could not record the end of a job");
    }
}

/// Runs `work` and records it as a job: `running`, then `finished`, or `failed` with the error. The result of
/// `work` is returned unchanged.
pub async fn track<T, F>(ctx: &AppContext, info: JobInfo, work: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    let id = match insert(&ctx.db, &info, queue_jobs::RUNNING, true).await {
        Ok(row) => Some(row.id),
        Err(error) => {
            tracing::warn!(%error, "could not record the start of a job");
            None
        }
    };
    let result = work.await;
    if let Some(id) = id {
        let outcome = result
            .as_ref()
            .map(|_| ())
            .map_err(|error| loco_rs::Error::string(&error.to_string()));
        finish(ctx, id, &outcome).await;
    }
    result
}

/// Like [`track`], for a job that was recorded earlier with [`queued`]: it is marked `running` (and its
/// attempt counted) before `work` starts.
pub async fn track_queued<T, F>(ctx: &AppContext, job_id: &str, work: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    let row = queue_jobs::Entity::find()
        .filter(queue_jobs::Column::JobId.eq(job_id))
        .one(&ctx.db)
        .await
        .ok()
        .flatten();
    let id = match row {
        Some(row) => {
            let now = Utc::now().fixed_offset();
            let attempt = row.attempt + 1;
            let id = row.id;
            let mut active: queue_jobs::ActiveModel = row.into();
            active.status = Set(queue_jobs::RUNNING.to_string());
            active.started_at = Set(Some(now));
            active.attempt = Set(attempt);
            active.updated_at = Set(Some(now));
            if let Err(error) = active.update(&ctx.db).await {
                tracing::warn!(%error, "could not record the start of a job");
            }
            Some(id)
        }
        None => None,
    };
    let result = work.await;
    if let Some(id) = id {
        let outcome = result
            .as_ref()
            .map(|_| ())
            .map_err(|error| loco_rs::Error::string(&error.to_string()));
        finish(ctx, id, &outcome).await;
    }
    result
}

/// Counts for the overview page.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Stats {
    pub queued: u64,
    pub running: u64,
    pub finished: u64,
    pub failed: u64,
    pub stale: u64,
    /// Rows in the Laravel-style `jobs` table (waiting).
    pub pending: u64,
    /// Rows in the Laravel-style `failed_jobs` table.
    pub failed_table: u64,
}

pub async fn stats<C: ConnectionTrait>(db: &C) -> Result<Stats, DbErr> {
    let count = |status: &'static str| async move {
        queue_jobs::Entity::find()
            .filter(queue_jobs::Column::Status.eq(status))
            .count(db)
            .await
    };
    Ok(Stats {
        queued: count(queue_jobs::QUEUED).await?,
        running: count(queue_jobs::RUNNING).await?,
        finished: count(queue_jobs::FINISHED).await?,
        failed: count(queue_jobs::FAILED).await?,
        stale: count(queue_jobs::STALE).await?,
        pending: jobs::Entity::find().count(db).await?,
        failed_table: failed_jobs::Entity::find().count(db).await?,
    })
}

/// Marks jobs that have been `running` for more than `hours` as `stale` (their worker probably died).
pub async fn mark_stale<C: ConnectionTrait>(db: &C, hours: i64) -> Result<u64, DbErr> {
    let cutoff = (Utc::now() - Duration::hours(hours)).fixed_offset();
    let result = queue_jobs::Entity::update_many()
        .col_expr(
            queue_jobs::Column::Status,
            sea_orm::sea_query::Expr::value(queue_jobs::STALE),
        )
        .filter(queue_jobs::Column::Status.eq(queue_jobs::RUNNING))
        .filter(queue_jobs::Column::StartedAt.lt(cutoff))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}

/// Deletes finished, failed and stale rows older than `days`. Waiting and running jobs are kept.
pub async fn purge<C: ConnectionTrait>(db: &C, days: i64) -> Result<u64, DbErr> {
    let cutoff = (Utc::now() - Duration::days(days)).fixed_offset();
    let result = queue_jobs::Entity::delete_many()
        .filter(queue_jobs::Column::CreatedAt.lt(cutoff))
        .filter(queue_jobs::Column::Status.is_in([
            queue_jobs::FINISHED,
            queue_jobs::FAILED,
            queue_jobs::STALE,
        ]))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}
