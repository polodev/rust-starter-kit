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
- This app has no database queue of its own, so Pending and Failed stay empty unless a Laravel app writes them.
