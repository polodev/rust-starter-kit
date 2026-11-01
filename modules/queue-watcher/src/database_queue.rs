//! A Laravel-style database queue on the `jobs` and `failed_jobs` tables (Laravel's own schema, so SQL exported
//! from a Laravel app imports as is). Write a job as a struct, dispatch it, and a worker runs it:
//!
//! ```ignore
//! #[derive(Serialize, Deserialize)]
//! pub struct SendDigest { pub user_id: u32 }
//!
//! #[async_trait]
//! impl DatabaseJob for SendDigest {
//!     const NAME: &'static str = "SendDigest";
//!     fn queue() -> &'static str { "emails" }
//!     async fn handle(self, ctx: &AppContext) -> Result<()> { /* the work */ Ok(()) }
//! }
//!
//! dispatch(SendDigest { user_id: 7 }).delay(30).send(&ctx.db).await?;   // SendDigest::dispatch(7)->delay(30)
//! ```
//!
//! The module that owns the job lists it in `Module::database_jobs` (`vec![database_job::<SendDigest>()]`). Run
//! workers with `cargo run -- task jobs:work queue:emails,default` (Supervisor in production).
//!
//! The stored payload uses Laravel's envelope (`uuid`, `displayName`, `job`, `maxTries`, `data`) plus
//! `"origin":"rust-starter"`. A Rust worker takes only rows with that mark, so it never runs a Laravel job it
//! cannot understand; keep Laravel and Rust jobs on different queue names so a Laravel worker does not take ours.

use std::{collections::HashMap, time::Duration};

use async_trait::async_trait;
use chrono::Utc;
use loco_database::{int, to_u64, to_u8, UnsignedInt};
use loco_modular::{DatabaseJobFuture, DatabaseJobHandler};
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Error, Result,
};
use sea_orm::{
    sea_query::{Expr, ExprTrait},
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    entities::{failed_jobs, jobs},
    tracker::{track, JobInfo},
};

/// The `connection` value written to `failed_jobs` (Laravel's queue connection name).
pub const CONNECTION: &str = "database";
const ORIGIN_MARK: &str = "\"origin\":\"rust-starter\"";
/// A job held by a worker that never reported back (it crashed) is taken over after this many seconds.
const RESERVATION_SECONDS: UnsignedInt = 300;
const POLL_EVERY: Duration = Duration::from_secs(2);
const BATCH: u64 = 20;

/// A job: its arguments are the fields of the struct (stored as JSON), its work is [`DatabaseJob::handle`].
#[async_trait]
pub trait DatabaseJob: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// Stable name stored in the payload (Laravel's `displayName`). Never rename a job that has waiting rows.
    const NAME: &'static str;

    /// The queue it goes to unless dispatched `on_queue`.
    fn queue() -> &'static str {
        "default"
    }

    /// Times it is tried in all before it moves to `failed_jobs` (Laravel's `$tries`).
    fn tries() -> u8 {
        3
    }

    /// Seconds to wait after the given failed attempt (1, 2, ...) before the next try (Laravel's `$backoff`).
    fn backoff_seconds(attempt: u8) -> u32 {
        if attempt <= 1 {
            60
        } else {
            300
        }
    }

    /// The work. An `Err` counts as a failed attempt.
    async fn handle(self, ctx: &AppContext) -> Result<()>;
}

fn run_job<J: DatabaseJob>(ctx: AppContext, arguments: Value) -> DatabaseJobFuture {
    Box::pin(async move {
        let job: J = serde_json::from_value(arguments)
            .map_err(|error| Error::string(&format!("bad arguments for {}: {error}", J::NAME)))?;
        job.handle(&ctx).await
    })
}

/// The `(name, handler)` pair a module returns from `Module::database_jobs`.
pub fn database_job<J: DatabaseJob>() -> (&'static str, DatabaseJobHandler) {
    (J::NAME, run_job::<J>)
}

/// A job waiting to be stored. Build it with [`dispatch`].
pub struct PendingDispatch<J: DatabaseJob> {
    job: J,
    queue: Option<String>,
    delay_seconds: u32,
    tries: Option<u8>,
}

/// Laravel's `Job::dispatch(...)`: `dispatch(job).on_queue("emails").delay(30).send(&ctx.db).await?`.
pub fn dispatch<J: DatabaseJob>(job: J) -> PendingDispatch<J> {
    PendingDispatch {
        job,
        queue: None,
        delay_seconds: 0,
        tries: None,
    }
}

impl<J: DatabaseJob> PendingDispatch<J> {
    /// Laravel's `->onQueue('emails')`.
    #[must_use]
    pub fn on_queue(mut self, queue: &str) -> Self {
        self.queue = Some(queue.to_string());
        self
    }

    /// Laravel's `->delay(30)`, in seconds.
    #[must_use]
    pub fn delay(mut self, seconds: u32) -> Self {
        self.delay_seconds = seconds;
        self
    }

    /// Laravel's `->tries(5)`.
    #[must_use]
    pub fn tries(mut self, tries: u8) -> Self {
        self.tries = Some(tries);
        self
    }

    /// Stores the job in `jobs` and returns its id. Use a transaction as `db` to dispatch together with your data.
    pub async fn send<C: ConnectionTrait>(self, db: &C) -> Result<u64> {
        let queue = self
            .queue
            .unwrap_or_else(|| J::queue().to_string())
            .chars()
            .take(255)
            .collect::<String>();
        let tries = std::cmp::max(self.tries.unwrap_or_else(J::tries), 1);
        let backoff: Vec<u32> = (1..tries).map(J::backoff_seconds).collect();
        let command =
            serde_json::to_string(&self.job).map_err(|error| Error::string(&error.to_string()))?;
        let payload = json!({
            "uuid": Uuid::new_v4().to_string(),
            "displayName": J::NAME,
            "job": "Illuminate\\Queue\\CallQueuedHandler@call",
            "maxTries": tries,
            "maxExceptions": null,
            "failOnTimeout": false,
            "backoff": backoff,
            "timeout": null,
            "retryUntil": null,
            "data": { "commandName": J::NAME, "command": command },
            "origin": "rust-starter",
            "attempts": 0,
        });
        let now = unix_now();
        let row = jobs::ActiveModel {
            queue: Set(queue),
            payload: Set(payload.to_string()),
            attempts: Set(0),
            reserved_at: Set(None),
            available_at: Set(now + int(self.delay_seconds)),
            created_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(to_u64(row.id))
    }
}

fn unix_now() -> UnsignedInt {
    int(u32::try_from(Utc::now().timestamp()).unwrap_or(u32::MAX))
}

/// What a worker needs from a stored payload.
struct Parsed {
    uuid: String,
    name: String,
    tries: u8,
    backoff: Vec<u32>,
    arguments: Value,
}

fn parse(payload: &str) -> Result<Parsed> {
    let value: Value = serde_json::from_str(payload)
        .map_err(|error| Error::string(&format!("payload is not JSON: {error}")))?;
    let name = value["displayName"]
        .as_str()
        .ok_or_else(|| Error::string("payload has no displayName"))?
        .to_string();
    let command = value["data"]["command"]
        .as_str()
        .ok_or_else(|| Error::string("payload has no data.command"))?;
    let arguments: Value = serde_json::from_str(command)
        .map_err(|error| Error::string(&format!("command is not JSON: {error}")))?;
    Ok(Parsed {
        uuid: value["uuid"]
            .as_str()
            .map_or_else(|| Uuid::new_v4().to_string(), str::to_string),
        name,
        tries: std::cmp::max(
            u8::try_from(value["maxTries"].as_u64().unwrap_or(3)).unwrap_or(3),
            1,
        ),
        backoff: value["backoff"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|item| item.as_u64().and_then(|n| u32::try_from(n).ok()))
                    .collect()
            })
            .unwrap_or_default(),
        arguments,
    })
}

type Handlers = HashMap<&'static str, DatabaseJobHandler>;

/// Runs the jobs that are due on `queues`, at most [`BATCH`]; returns how many were handled (finished, released
/// for a later try, or failed for good).
pub async fn work_once(ctx: &AppContext, handlers: &Handlers, queues: &[String]) -> Result<usize> {
    let db = &ctx.db;
    let now = unix_now();
    let stale = now.saturating_sub(RESERVATION_SECONDS);
    let free = || {
        Condition::any()
            .add(jobs::Column::ReservedAt.is_null())
            .add(jobs::Column::ReservedAt.lt(stale))
    };
    let due = jobs::Entity::find()
        .filter(jobs::Column::Queue.is_in(queues.iter().cloned()))
        .filter(jobs::Column::AvailableAt.lte(now))
        .filter(free())
        .filter(jobs::Column::Payload.contains(ORIGIN_MARK))
        .order_by_asc(jobs::Column::Id)
        .limit(BATCH)
        .all(db)
        .await?;

    let mut handled = 0;
    for entry in due {
        // Several workers may run; only the one whose update changes the row owns the job.
        let claimed = jobs::Entity::update_many()
            .col_expr(jobs::Column::ReservedAt, Expr::value(now))
            .col_expr(
                jobs::Column::Attempts,
                Expr::col(jobs::Column::Attempts).add(1),
            )
            .filter(jobs::Column::Id.eq(entry.id))
            .filter(free())
            .exec(db)
            .await?;
        if claimed.rows_affected == 1 {
            attempt(ctx, handlers, entry).await?;
            handled += 1;
        }
    }
    Ok(handled)
}

/// One try of a claimed job: delete it when it works, release it with a back-off, or move it to `failed_jobs`.
async fn attempt(ctx: &AppContext, handlers: &Handlers, entry: jobs::Model) -> Result<()> {
    let db = &ctx.db;
    let attempts = to_u8(entry.attempts).saturating_add(1);
    let parsed = match parse(&entry.payload) {
        Ok(parsed) => parsed,
        Err(error) => return fail(db, &entry, None, &error.to_string()).await,
    };

    let outcome = match handlers.get(parsed.name.as_str()) {
        None => Err(Error::string(&format!(
            "no handler is registered for job {}",
            parsed.name
        ))),
        Some(handler) => {
            let mut info = JobInfo::new(&parsed.name)
                .queue(&entry.queue)
                .payload(parsed.arguments.clone());
            info.connection = CONNECTION.to_string();
            // Run the job on its own task, so a panic fails this try instead of stopping the worker.
            let running = tokio::spawn(handler(ctx.clone(), parsed.arguments.clone()));
            track(ctx, info, async {
                running
                    .await
                    .map_err(|error| Error::string(&format!("the job panicked: {error}")))?
            })
            .await
        }
    };

    match outcome {
        Ok(()) => {
            jobs::Entity::delete_by_id(entry.id).exec(db).await?;
        }
        Err(error) if attempts >= parsed.tries => {
            fail(db, &entry, Some(&parsed.uuid), &error.to_string()).await?;
        }
        Err(error) => {
            tracing::warn!(job = %parsed.name, attempts, %error, "database job failed, will be tried again");
            let wait = parsed
                .backoff
                .get(usize::from(attempts) - 1)
                .or_else(|| parsed.backoff.last())
                .copied()
                .unwrap_or(60);
            let mut release: jobs::ActiveModel = entry.into();
            release.reserved_at = Set(None);
            release.available_at = Set(unix_now() + int(wait));
            release.update(db).await?;
        }
    }
    Ok(())
}

/// Moves a job to `failed_jobs` (Laravel's table) and removes it from `jobs`.
async fn fail<C: ConnectionTrait>(
    db: &C,
    entry: &jobs::Model,
    uuid: Option<&String>,
    exception: &str,
) -> Result<()> {
    failed_jobs::ActiveModel {
        uuid: Set(uuid.cloned().unwrap_or_else(|| Uuid::new_v4().to_string())),
        connection: Set(CONNECTION.to_string()),
        queue: Set(entry.queue.clone()),
        payload: Set(entry.payload.clone()),
        exception: Set(exception.chars().take(20_000).collect()),
        failed_at: Set(Utc::now().fixed_offset()),
        ..Default::default()
    }
    .insert(db)
    .await?;
    jobs::Entity::delete_by_id(entry.id).exec(db).await?;
    Ok(())
}

/// Puts failed jobs back on their queue (`which` is `all`, a numeric id or a uuid); returns how many.
pub async fn retry_failed<C: ConnectionTrait>(db: &C, which: &str) -> Result<usize> {
    let mut query = failed_jobs::Entity::find().order_by_asc(failed_jobs::Column::Id);
    if which != "all" {
        query = match which.parse::<u64>() {
            Ok(id) => query.filter(failed_jobs::Column::Id.eq(id)),
            Err(_) => query.filter(failed_jobs::Column::Uuid.eq(which)),
        };
    }
    let rows = query.all(db).await?;
    let now = unix_now();
    for row in &rows {
        jobs::ActiveModel {
            queue: Set(row.queue.clone()),
            payload: Set(row.payload.clone()),
            attempts: Set(0),
            reserved_at: Set(None),
            available_at: Set(now),
            created_at: Set(now),
            ..Default::default()
        }
        .insert(db)
        .await?;
        failed_jobs::Entity::delete_by_id(row.id).exec(db).await?;
    }
    Ok(rows.len())
}

/// Deletes every failed job; returns how many (Laravel's `queue:flush`).
pub async fn flush_failed<C: ConnectionTrait>(db: &C) -> Result<u64> {
    Ok(failed_jobs::Entity::delete_many()
        .exec(db)
        .await?
        .rows_affected)
}

/// The worker loop: runs due jobs on `queues` forever. Used by `jobs:work` and the embedded worker.
pub async fn run_worker(ctx: &AppContext, handlers: &Handlers, queues: &[String]) -> Result<()> {
    loop {
        match work_once(ctx, handlers, queues).await {
            Ok(0) => tokio::time::sleep(POLL_EVERY).await,
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, "database queue worker pass failed");
                tokio::time::sleep(POLL_EVERY).await;
            }
        }
    }
}

fn setting_text(ctx: &AppContext, key: &str) -> Option<String> {
    let value = ctx
        .config
        .settings
        .as_ref()?
        .get("database_queue")?
        .get(key)?;
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn queue_names(text: &str) -> Vec<String> {
    let names: Vec<String> = text
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect();
    if names.is_empty() {
        vec!["default".to_string()]
    } else {
        names
    }
}

/// Starts a worker inside this process when `settings.database_queue.embedded_worker` is on (development).
/// Production runs `jobs:work` under Supervisor instead.
pub fn spawn_embedded_worker(ctx: &AppContext, handlers: Handlers) {
    let enabled = setting_text(ctx, "embedded_worker")
        .is_some_and(|text| matches!(text.trim().to_lowercase().as_str(), "true" | "1" | "yes"));
    if !enabled {
        return;
    }
    let queues = queue_names(&setting_text(ctx, "queues").unwrap_or_default());
    let ctx = ctx.clone();
    tokio::spawn(async move {
        tracing::info!(
            ?queues,
            "database queue worker started inside the web server"
        );
        if let Err(error) = run_worker(&ctx, &handlers, &queues).await {
            tracing::error!(%error, "database queue worker stopped");
        }
    });
}

/// `jobs:work`: runs database jobs. Register it with the handlers of every module (see `src/app.rs`).
pub struct JobsWork {
    handlers: Handlers,
}

impl JobsWork {
    pub fn new(handlers: Handlers) -> Self {
        Self { handlers }
    }
}

#[async_trait]
impl Task for JobsWork {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "jobs:work".to_string(),
            detail: "Run database queue jobs (Laravel's queue:work).\nUsage: cargo run -- task jobs:work queue:emails,default [once:true]".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let queues = queue_names(
            &vars
                .cli_arg("queue")
                .map(str::to_string)
                .ok()
                .or_else(|| setting_text(ctx, "queues"))
                .unwrap_or_default(),
        );
        if vars.cli_arg("once").is_ok_and(|value| value == "true") {
            let handled = work_once(ctx, &self.handlers, &queues).await?;
            println!("handled {handled} job(s) on {}", queues.join(","));
            return Ok(());
        }
        println!("working {} (Ctrl+C to stop)", queues.join(","));
        run_worker(ctx, &self.handlers, &queues).await
    }
}

/// `jobs:failed`: lists failed jobs (Laravel's `queue:failed`).
pub struct JobsFailed;

#[async_trait]
impl Task for JobsFailed {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "jobs:failed".to_string(),
            detail: "List failed database jobs.".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
        let rows = failed_jobs::Entity::find()
            .order_by_asc(failed_jobs::Column::Id)
            .all(&ctx.db)
            .await?;
        if rows.is_empty() {
            println!("no failed jobs");
        }
        for row in rows {
            let first_line = row.exception.lines().next().unwrap_or_default();
            println!(
                "{}\t{}\t{}\t{}\t{}",
                row.id, row.uuid, row.queue, row.failed_at, first_line
            );
        }
        Ok(())
    }
}

/// `jobs:retry-failed`: puts failed jobs back on their queue (Laravel's `queue:retry`).
pub struct JobsRetryFailed;

#[async_trait]
impl Task for JobsRetryFailed {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "jobs:retry-failed".to_string(),
            detail: "Retry failed database jobs.\nUsage: cargo run -- task jobs:retry-failed which:all|<id>|<uuid>".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let which = vars
            .cli_arg("which")
            .map_err(|_| Error::string("pass which:all, which:<id> or which:<uuid>"))?;
        let count = retry_failed(&ctx.db, which).await?;
        println!("{count} job(s) put back on their queue");
        Ok(())
    }
}

/// `jobs:flush-failed`: deletes every failed job (Laravel's `queue:flush`).
pub struct JobsFlushFailed;

#[async_trait]
impl Task for JobsFlushFailed {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "jobs:flush-failed".to_string(),
            detail: "Delete all failed database jobs.".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
        println!("{} failed job(s) deleted", flush_failed(&ctx.db).await?);
        Ok(())
    }
}
