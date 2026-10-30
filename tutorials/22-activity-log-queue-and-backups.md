<!-- nav:start -->
**Tutorial 22 of 25** | [Index](00-index.md) | [Previous: 21 Conventions](21-conventions.md) | [Next: 23 PDF and email](23-pdf-and-email.md)

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
**22. [Activity log, queue monitor and database backups](22-activity-log-queue-and-backups.md)**  
23. [PDF and email](23-pdf-and-email.md)  
24. [Email and PDF step by step](24-email-and-pdf-step-by-step.md)  
25. [AI service: Claude, OpenAI and Gemini](25-ai-service.md)  

</details>
<!-- nav:end -->

# 22. Activity log, queue monitor and database backups

Three small operations modules. Each has its own tables (or none), admin screens or commands, and tests.

## Activity log (`activity-log`)

Spatie laravel-activitylog's `activity_log` table, so a Laravel app can share the log. Write an entry from any module:

```rust
use loco_activity_log::{activity, changes};

activity(&ctx.db)
    .in_log("users")                         // inLog('users')
    .caused_by("user", admin.id)             // causedBy($admin)
    .performed_on("user", target.id)         // performedOn($target)
    .event("updated")
    .with_changes(changes(&old, &new, &[]))  // {"attributes": {...}, "old": {...}}, only what changed
    .log_quietly("Updated the user's details")   // never breaks the request
    .await;
```

- **Where it is written:** sign in and out, registration, account changes, user edits, roles, page text, publish, and contact replies.
- **Admin:** `/admin/activity-log` (`activity_logs.view`) has log, event and text filters; the ID opens a before/after table. `activity_logs.manage` deletes entries and cleans old ones.
- **Clean:** `cargo run -- task activitylog:clean days:90 log:users` (default 365 days, scheduled daily).
- **Rules:** never log secrets; set `ACTIVITY_LOGGER_ENABLED=false` to switch it off. The writer is the `loco-activity-log` package so lower modules can log without a dependency cycle.

## Queue monitor (`queue-watcher`)

`/admin/queue-watcher` (`queue.view` reads, `queue.manage` deletes):

| Tab | Shows |
|---|---|
| Overview | counts by status and the newest jobs |
| Tracked jobs | this app's background jobs: `queued`, `running`, `finished`, `failed`, `stale`, with time, error, trace and payload |
| Pending jobs | a Laravel queue's waiting `jobs` table: show, delete |
| Failed jobs | its `failed_jobs` table: show, delete, delete all |

Background jobs here run inside the app, so a worker records itself:

```rust
track(&self.ctx, JobInfo::new("SendDigest").queue("mail"), async {
    // the real work; an Err is recorded as failed and returned unchanged
    Ok(())
}).await
```

`queue-watcher:mark-stale` (hourly) flags jobs that ran too long; `queue-watcher:purge days:14` (daily) removes old history.

## Database backups (`db-backup`)

Like `php artisan backup:run --only-db`, for the database only:

```sh
cargo run -- task backup:run only-db:true      # dump (gzip) and store on every destination
cargo run -- task backup:run disk:r2 filename:before-upgrade
cargo run -- task backup:list
cargo run -- task backup:clean dry-run:true    # Spatie's retention: all 7 days, then daily, weekly, monthly, yearly
cargo run -- task backup:monitor               # non-zero exit if a destination has no recent backup
```

Backups go to disks configured in `settings.db_backup`, **separate from the website's storage**, so they can live in another bucket or account. The shipped config reads everything from `.env`, so turning it on is:

```sh
DB_BACKUP_ENABLED=true
BACKUP_BUCKET=my-private-bucket
BACKUP_ENDPOINT=https://<account>.r2.cloudflarestorage.com   # R2, S3, MinIO, B2 ...
BACKUP_ACCESS_KEY_ID=...
BACKUP_SECRET_ACCESS_KEY=...
```

The app reads `.env` at startup. Use a private bucket. With the switch off, nothing runs and nothing is checked. Other disk types (a local folder, several destinations) are in the module README.

It needs `mysqldump` on the machine. The password goes to the dump through the environment, never the command line. The scheduler runs clean (01:00), run (01:30) and monitor (03:00) daily, and does nothing until the settings exist. Restore: `gunzip -c file.sql.gz | mysql -u root my_database`. Full settings: `modules/db-backup/README.md`.

<!-- foot:start -->
---
[Previous: 21 Conventions](21-conventions.md) | [Index](00-index.md) | [Next: 23 PDF and email](23-pdf-and-email.md)
<!-- foot:end -->
