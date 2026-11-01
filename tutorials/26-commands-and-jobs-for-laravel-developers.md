<!-- nav:start -->
**Tutorial 26 of 28** | [Index](00-index.md) | [Previous: 25 AI service: Claude, OpenAI and Gemini](25-ai-service.md) | [Next: 27 PostgreSQL for MySQL users](27-postgresql-for-mysql-users.md)

<details><summary>Jump to any tutorial</summary>

[00. Index](00-index.md)  
01. [Introduction](01-introduction.md)  
02. [Setup](02-setup.md)  
03. [Rust essentials](03-rust-essentials.md)  
04. [Project structure](04-project-structure.md)  
05. [Modules and `cargo modular`](05-modules.md)  
06. [Routes, controllers and views](06-routes-controllers-views.md)  
07. [Database: migrations, entities, queries, seeding](07-database.md)  
08. [A bilingual site (English and Bangla)](08-bilingual-site.md)  
09. [Cache and sessions](09-cache-and-sessions.md)  
10. [Testing](10-testing.md)  
11. [Assets and styling](11-assets-and-styling.md)  
12. [Cheat sheet and what to build next](12-cheatsheet.md)  
13. [Console commands and the scheduler](13-console-and-schedule.md)  
14. [Login, register and sessions (`auth` module)](14-auth-and-sessions.md)  
15. [Roles and permissions (`role-permission` module)](15-roles-and-permissions.md)  
16. [The admin dashboard (`admin-layout` module)](16-admin-dashboard.md)  
17. [Media library (`media` module)](17-media-library.md)  
18. [JSON API and Google login](18-json-api-and-google-login.md)  
19. [Components and the contact form](19-components-and-contact-form.md)  
20. [Tinker and sharing the database with Laravel](20-tinker-and-laravel-database.md)  
21. [Conventions](21-conventions.md)  
22. [Activity log, queue monitor and database backups](22-activity-log-queue-and-backups.md)  
23. [PDF and email](23-pdf-and-email.md)  
24. [Email and PDF step by step](24-email-and-pdf-step-by-step.md)  
25. [AI service: Claude, OpenAI and Gemini](25-ai-service.md)  
**26. [Commands and jobs for Laravel developers](26-commands-and-jobs-for-laravel-developers.md)**  
27. [PostgreSQL for MySQL users](27-postgresql-for-mysql-users.md)  
28. [MySQL or PostgreSQL, one choice per app](28-mysql-or-postgresql-per-app.md)  

</details>
<!-- nav:end -->

# 26. Commands and jobs, for Laravel developers

You know `php artisan make:command`, `php artisan make:job`, `dispatch()`, `queue:work` and Supervisor. This page
maps each of them to this project, says what Loco gives you, and says plainly what is **not** built yet.

## The short version

| Laravel | Here | Status |
|---|---|---|
| `php artisan make:command` | `cargo modular make:command Name --module=x` | works |
| `php artisan my:command` | `cargo run -- task my:command key:value` | works |
| `Schedule::command()->dailyAt()` | `ScheduledJob::command("x:y").daily_at(3, 0)` | works |
| `php artisan make:mail` | `cargo modular make:mail Name --module=x` | works |
| `Mail::queue()` / queued mail | `module_mail::dispatch(&ctx, mail)` (database queue, default) | works |
| `queue:work` for mail | `task mail:work` (Supervisor program) | works |
| `php artisan make:job` + `Job::dispatch()` | Loco `BackgroundWorker` + `perform_later` | works, saved in Redis |
| `QUEUE_CONNECTION=database`: `jobs` and `failed_jobs`, `queue:work --queue=a,b` | `DatabaseJob`, `dispatch(..)`, `task jobs:work queue:a,b` | works (MariaDB, section 5) |
| `QUEUE_CONNECTION=redis` | Loco workers, `start --worker a,b` | works (Redis, section 4) |

Why Redis for jobs: Loco's persisted queue runs on Redis, Postgres or SQLite, not MariaDB. This project uses Redis. Mail has its own MariaDB queue (`mail_queue`).

## 1. A command (Laravel: `make:command`)

```sh
cargo modular make:command SendDigest --module=blog       # signature blog:send-digest
cargo modular make:command SendDigest --module=blog --signature=emails:digest
```

It creates `modules/blog/src/commands/send_digest.rs` and registers it. A command is a Loco `Task`:

```rust
pub struct SendDigest;

#[async_trait]
impl Task for SendDigest {
    fn task(&self) -> TaskInfo {
        TaskInfo { name: "blog:send-digest".to_string(), detail: "Email the weekly digest.".to_string() }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let dry_run = vars.cli_arg("dry-run").is_ok_and(|value| value == "true");   // $this->option('dry-run')
        // ctx.db is your DB facade, ctx.cache is Cache, ctx.config is config()
        println!("done");                                                          // $this->info('done')
        Ok(())                                                                     // Err(..) = non-zero exit
    }
}
```

Run it and schedule it:

```sh
cargo run -- task blog:send-digest dry-run:true      # php artisan blog:send-digest --dry-run
cargo modular command:list                           # php artisan list
```

```rust
fn schedule(&self) -> Vec<ScheduledJob> {            // routes/console.php
    vec![ScheduledJob::command("blog:send-digest").daily_at(8, 0)]
}
```

`cargo modular schedule:list` shows it, and the process started with `start --scheduler` runs it (one scheduler only,
like a single `schedule:work`). More in [tutorial 13](13-console-and-schedule.md).

## 2. A job (Laravel: `make:job`)

A Loco job is a **worker**: a struct with typed arguments and a `perform` method. The arguments are your job's
constructor properties.

```sh
cargo loco generate worker send_digest_to_user
```

This writes `src/workers/send_digest_to_user.rs` and registers it. Edit it:

```rust
#[derive(Deserialize, Debug, Serialize)]
pub struct SendDigestToUserArgs {
    pub user_id: u32,                 // public function __construct(public int $userId)
}

pub struct SendDigestToUser { pub ctx: AppContext }

#[async_trait]
impl BackgroundWorker<SendDigestToUserArgs> for SendDigestToUser {
    fn build(ctx: &AppContext) -> Self { Self { ctx: ctx.clone() } }

    async fn perform(&self, args: SendDigestToUserArgs) -> Result<()> {     // public function handle()
        // Wrap the body so the run appears in /admin/queue-watcher with status, time and error.
        let info = JobInfo::new("SendDigestToUser").queue("default");
        module_queue_watcher::track(&self.ctx, info, async {
            // ... the work ...
            Ok(())
        })
        .await                                                              // Err(..) = the job failed
    }
}
```

Dispatch it from a handler:

```rust
SendDigestToUser::perform_later(&ctx, SendDigestToUserArgs { user_id: 7 }).await?;   // SendDigestToUser::dispatch(7)
```

### Other queues (Laravel: `->onQueue('emails')`)

A worker chooses its queue by overriding one function, which is the same as `public $queue = 'emails'`:

```rust
fn queue() -> Option<String> { Some("emails".to_string()) }
```

A worker process listens to chosen queues (`start --worker emails,default`), like `queue:work --queue=emails,default`.

## 3. What Loco gives you, and what it does not

Gives you (use these, do not add crates):

- the `Task` trait, CLI arguments, the scheduler and the test harness (a command is a function you can call in a test)
- `BackgroundWorker` with typed, serialisable arguments, a `queue()` name and priorities, `perform_later_with_priority`
- `AppContext`: database, cache, mailer, storage and config in one value, like Laravel's container
- `ForegroundBlocking` mode for tests, so a dispatched job has finished when the next line runs

Does **not** give you (Loco's queue needs Redis, so the database queue in section 5 is our own code):

- a MariaDB queue (we built `DatabaseJob` for that)
- retry buttons on the Failed screen (use `jobs:retry-failed`)
- a `make:job` generator (copy the example in section 5)

## 4. Persistent jobs and named queues (Redis)

Jobs are stored in Redis, so they survive an app restart and several worker processes can share them. **Redis must be
running** (`redis-cli ping` answers `PONG`). The main database stays MariaDB.

```yaml
# config/development.yaml and production.yaml
workers:
  mode: BackgroundQueue                  # WORKERS_MODE; tests use ForegroundBlocking
queue:
  kind: Redis
  uri: <QUEUE_REDIS_URL>                 # .env: redis://127.0.0.1:6379/5   (redis://:password@host:6379/5 in production)
```

| Laravel | Here |
|---|---|
| `QUEUE_CONNECTION=redis`, `REDIS_DB=5` | `queue: kind: Redis` and the number at the end of `QUEUE_REDIS_URL` |
| `->onQueue('emails')` | `fn queue() -> Option<String> { Some("emails".into()) }` on the worker |
| `queue:work --queue=emails,default` | `cargo run -- start --worker emails,default` |
| `php artisan serve` + `queue:work` | `cargo run -- start --server-and-worker` |
| Supervisor `queue:work` | `[program:app-queue-worker]` in `deploy/supervisor.conf` |
| `php artisan horizon` | the queue monitor `/admin/queue-watcher` (history of tracked jobs) |

A Redis "database" is a number from 0 to 15, not a name. With many apps on one server give each its own number and keep a
table of them. Never run `FLUSHALL` on a shared Redis. Without a name, a worker listens to `default`.

Redis keeps jobs in memory and saves snapshots, so for jobs you cannot lose turn on `appendonly yes` in its config.
Mail keeps its own MariaDB queue (`mail_queue`, `task mail:work`), so it works even when Redis is down.

To look at the data, use `redis-cli -n 5 keys '*'` or a viewer such as TablePlus or RedisInsight.

## 5. Database jobs, the Laravel way (MariaDB)

This is `QUEUE_CONNECTION=database`. The tables are Laravel's own `jobs` and `failed_jobs`, so SQL exported from a Laravel
app imports as is and the queue monitor (Pending and Failed screens) shows them.

**Write the job** (Laravel: `php artisan make:job SendDigest`):

```rust
#[derive(Serialize, Deserialize)]
pub struct SendDigest { pub user_id: u32 }                      // public function __construct(public int $userId)

#[async_trait]
impl DatabaseJob for SendDigest {
    const NAME: &'static str = "SendDigest";                    // the class name; never rename while rows wait
    fn queue() -> &'static str { "emails" }                     // public $queue = 'emails'
    fn tries() -> u8 { 3 }                                      // public $tries = 3
    fn backoff_seconds(attempt: u8) -> u32 { if attempt <= 1 { 60 } else { 300 } }   // public $backoff
    async fn handle(self, ctx: &AppContext) -> Result<()> {     // public function handle()
        // send the digest for self.user_id; an Err counts as a failed try
        Ok(())
    }
}
```

**Register it** in the module that owns it (like adding it to a service provider):

```rust
fn database_jobs(&self) -> Vec<(&'static str, DatabaseJobHandler)> {
    vec![module_queue_watcher::database_job::<SendDigest>()]
}
```

**Dispatch it** from a handler or command:

```rust
dispatch(SendDigest { user_id: 7 }).send(&ctx.db).await?;                                   // SendDigest::dispatch(7)
dispatch(SendDigest { user_id: 7 }).on_queue("emails").delay(30).tries(5).send(&ctx.db).await?;
```

Pass a transaction instead of `&ctx.db` and the job exists only if your data was saved, which Redis cannot do.

**Run the worker** and manage failures:

| Laravel | Here |
|---|---|
| `php artisan queue:work --queue=emails,default` | `cargo run -- task jobs:work queue:emails,default` |
| `php artisan queue:work --once` | `cargo run -- task jobs:work once:true` |
| `php artisan queue:failed` | `cargo run -- task jobs:failed` |
| `php artisan queue:retry all` / `queue:retry <uuid>` | `cargo run -- task jobs:retry-failed which:all` (or an id or uuid) |
| `php artisan queue:flush` | `cargo run -- task jobs:flush-failed` |
| Supervisor `queue:work` | `[program:app-jobs-worker]` in `deploy/supervisor.conf`, one per group of queues |

In development a worker runs inside the web server (`JOBS_EMBEDDED_WORKER=true`, queues in `JOBS_QUEUES`), so nothing else
needs to start. After a deploy restart the worker: `supervisorctl restart app-jobs-worker`.

How it behaves: two workers never take the same job; a failed try waits for the back-off (default 1 then 5 minutes); after
the last try the job and its error move to `failed_jobs`; a job held by a crashed worker is taken over after 5 minutes; each
run appears in the queue monitor's tracked history.

**Sharing the database with a Laravel app.** Our payload is Laravel's envelope (`uuid`, `displayName`, `job`, `maxTries`,
`data`) plus `"origin":"rust-starter"`. The Rust worker takes only rows with that mark, so it never touches a Laravel job
(they are PHP classes it cannot run), and a Laravel worker cannot run ours. Use separate queue names for the two apps.

**Choosing:** database jobs when a job must be saved together with your data, must work without Redis, or the database is
shared with Laravel; Loco workers on Redis for high volume. The database queue polls every two seconds, so a job starts
within about two seconds.

[Index](00-index.md)

<!-- foot:start -->
---
[Previous: 25 AI service: Claude, OpenAI and Gemini](25-ai-service.md) | [Index](00-index.md) | [Next: 27 PostgreSQL for MySQL users](27-postgresql-for-mysql-users.md)
<!-- foot:end -->
