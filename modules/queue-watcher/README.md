# queue-watcher

A queue monitor under `/admin/queue-watcher`. It shows the history of this app's background jobs, and the
waiting and failed jobs of a Laravel queue that shares the database, and lets staff delete them.

| Screen | Data | Actions |
|---|---|---|
| Overview | counts by status, newest jobs | mark stale, purge |
| Tracked jobs | `queue_jobs`: status `queued`, `running`, `finished`, `failed`, `stale`; time taken; error and trace; payload | show, delete, purge, mark stale |
| Pending jobs | Laravel's `jobs` table (waiting) | show payload, delete |
| Failed jobs | Laravel's `failed_jobs` table | show exception, delete, delete all |

`queue.view` reads; `queue.manage` deletes and purges.

## Database jobs (Laravel's `jobs` and `failed_jobs` tables)

A Laravel-style database queue on the same tables the Pending and Failed screens show. The tables are Laravel's own
schema, so SQL exported from a Laravel app imports as is, and the screens list those jobs.

```rust
#[derive(Serialize, Deserialize)]
pub struct SendDigest { pub user_id: u32 }                  // the constructor properties

#[async_trait]
impl DatabaseJob for SendDigest {                           // the job class
    const NAME: &'static str = "SendDigest";                // never rename while rows wait
    fn queue() -> &'static str { "emails" }                 // public $queue
    fn tries() -> u8 { 3 }                                  // public $tries
    fn backoff_seconds(attempt: u8) -> u32 { if attempt <= 1 { 60 } else { 300 } }   // public $backoff
    async fn handle(self, ctx: &AppContext) -> Result<()> { Ok(()) }                 // handle()
}

// in the module that owns the job:
fn database_jobs(&self) -> Vec<(&'static str, DatabaseJobHandler)> { vec![database_job::<SendDigest>()] }

// anywhere with a database connection (a transaction works too):
dispatch(SendDigest { user_id: 7 }).on_queue("emails").delay(30).tries(5).send(&ctx.db).await?;
```

| Laravel | Here |
|---|---|
| `SendDigest::dispatch($id)->onQueue('emails')->delay(30)` | `dispatch(SendDigest { .. }).on_queue("emails").delay(30).send(&db)` |
| `php artisan queue:work --queue=emails,default` | `cargo run -- task jobs:work queue:emails,default` (`once:true` handles what is due and exits) |
| `queue:failed`, `queue:retry all`, `queue:flush` | `jobs:failed`, `jobs:retry-failed which:all` (or an id or uuid), `jobs:flush-failed` |
| `failed_jobs` after `$tries` | same: the job and its exception move to `failed_jobs` |

How it behaves: a worker claims a row with an atomic update (two workers never run the same job), runs `handle`, and
deletes the row. A failure releases the row after the back-off; after the last try it moves to `failed_jobs` with the
error. A job held by a crashed worker is taken over after 5 minutes. Every run is recorded in the tracked jobs history.
In development a worker runs inside the web server (`JOBS_EMBEDDED_WORKER=true`, queues in `JOBS_QUEUES`); in
production run `jobs:work` under Supervisor (`deploy/supervisor.conf`), one program per group of queues.

Sharing the database with a Laravel app: the payload uses Laravel's envelope plus `"origin":"rust-starter"`. A Rust
worker takes only rows with that mark, so it never touches a Laravel job it cannot run (PHP classes), and Laravel
cannot run ours. Give Rust jobs their own queue names so a Laravel `queue:work` does not pick them up.

## Record a worker

Background jobs here run inside the app, so they record themselves. Wrap the work:

```rust
use module_queue_watcher::{track, JobInfo};

async fn perform(&self, args: MyArgs) -> Result<()> {
    track(&self.ctx, JobInfo::new("SendDigest").queue("mail"), async {
        // the real work; an Err is recorded as failed and returned unchanged
        Ok(())
    })
    .await
}
```

`queued(ctx, &info)` records a job when it is dispatched and `track_queued(ctx, &job_id, work)` runs it later.
A problem writing the history never fails the job itself.

## Commands

- `queue-watcher:mark-stale hours:1`: jobs running longer than that are marked `stale` (hourly).
- `queue-watcher:purge days:14`: removes finished, failed and stale history older than that (daily).

## Notes

- `queue_jobs` keeps the same columns as the Laravel "queue watcher" table, and `jobs` / `failed_jobs` are
  Laravel's own tables, created with `IF NOT EXISTS`, so a Laravel app can use the same database.
- Pending and Failed show the database jobs above and any Laravel app's jobs. Jobs of Loco workers (Redis) are not in these tables, but their runs appear in the tracked history when wrapped in `track`. Mail has its own queue (`mail_queue`).
