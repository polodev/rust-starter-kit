<!-- nav:start -->
**Tutorial 13 of 17** | [Index](00-index.md) | [Previous: 12 Cheat sheet and what to build next](12-cheatsheet.md) | [Next: 14 Login, register and sessions (`auth` module)](14-auth-and-sessions.md)

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
**13. [Console commands and the scheduler](13-console-and-schedule.md)**  
14. [Login, register and sessions (`auth` module)](14-auth-and-sessions.md)  
15. [Roles and permissions (`role-permission` module)](15-roles-and-permissions.md)  
16. [The admin dashboard (`admin-layout` module)](16-admin-dashboard.md)  
17. [Media library (`media` module)](17-media-library.md)  

</details>
<!-- nav:end -->

# 13. Console commands and the scheduler

This is the Laravel `artisan` + `Schedule` experience. Under the hood these are Loco **tasks** and the Loco **scheduler**; `cargo modular` adds the Laravel-style commands on top.

| Laravel | Here |
|---|---|
| `php artisan make:command` | `cargo modular make:command CheckTranslations --module=pages` |
| `php artisan list` | `cargo modular command:list` |
| `php artisan emails:send` | `cargo modular command:run emails:send` |
| `Schedule::command(...)->dailyAt('03:00')` | `ScheduledJob::command("...").daily_at(3, 0)` |
| `php artisan schedule:list` | `cargo modular schedule:list` |
| `php artisan schedule:work` | `cargo modular schedule:work` |

## Make a command

```sh
cargo modular make:command SendDigest --module=blog
# signature defaults to <module>:<name>  ->  blog:send-digest
cargo modular make:command SendDigest --module=blog --signature=emails:digest
cargo modular make:command SendDigest --module=blog --dry-run   # preview only
```

This creates `modules/blog/src/commands/send_digest.rs` and registers it in `modules/blog/src/commands/mod.rs` (the generator edits the two `inject` marker lines; keep them).

Without `--module`, the request goes to Loco's own `generate task`, so app-level commands stay standard Loco.

## Write the command

A command implements Loco's `Task` trait:

```rust
pub struct CheckTranslations;

#[async_trait]
impl Task for CheckTranslations {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "pages:check-translations".to_string(),
            detail: "List published pages that miss a translation.".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let strict = vars.cli_arg("strict").is_ok_and(|value| value == "true");
        // use ctx.db, ctx.cache, ctx.config ... exactly like a controller
        Ok(())
    }
}
```

- Arguments arrive as `key:value` pairs: `cargo modular command:run pages:check-translations strict:true`.
- Return `Err(...)` to make the process exit non-zero. The scheduler logs the failure.
- Print with `println!` for people, `tracing::info!` for logs.

Run it:

```sh
cargo modular command:list
cargo modular command:run pages:check-translations strict:true
# the same thing without the helper:
cargo run -- task pages:check-translations strict:true
```

## Schedule it

A module declares its schedule in code, next to the command:

```rust
fn schedule(&self) -> Vec<ScheduledJob> {
    vec![ScheduledJob::command("pages:check-translations").daily_at(3, 0).tag("content")]
}
```

| Helper | Cron (sec min hour dom mon dow) |
|---|---|
| `every_minute()` | `0 * * * * *` |
| `every_five_minutes()` | `0 */5 * * * *` |
| `every_fifteen_minutes()` | `0 */15 * * * *` |
| `hourly()` | `0 0 * * * *` |
| `daily_at(3, 0)` | `0 0 3 * * *` |
| `weekly_on(1, 9, 30)` | `0 30 9 * * 1` (Monday 09:30) |
| `cron("...")` | any expression |

Other options: `.named("job-name")`, `.tag("x")`, `.run_on_start()`. Arguments go in the command string: `ScheduledJob::command("emails:digest limit:50")`.

At boot, `App::load_config` merges every module's jobs into the scheduler config. Jobs you write by hand in `config/*.yaml` under `scheduler:` still work, and two modules cannot use the same job name (the app refuses to start).

```sh
cargo modular schedule:list      # show all jobs and their cron
cargo modular schedule:work      # run the scheduler in the foreground
```

## Tinker: change the database from a prompt

Laravel's `php artisan tinker` evaluates PHP as you type it. Rust is compiled, so it cannot evaluate Rust that way. This project gives you two consoles instead:

| Command | What it is |
|---|---|
| `cargo run -- task tinker` | Built-in SQL console. Type SQL ending with `;`. Dot commands: `.tables`, `.desc users`, `.count users`, `.hash secret`, `.begin`, `.commit`, `.rollback`, `.help`, `.exit`. |
| `cargo run -- task tinker:eloquent` | The `rust-eloquent-tinker` module: real PsySH with Eloquent models and the `DB` facade on the same database. |

```text
tinker> update users set name = 'Ada' where id = 1;
1 row(s) affected
tinker> update users set name = 'x';
This has no WHERE and changes every row.
run it anyway? [y/N]
```

Use `.begin` first to try a change safely: nothing is saved until `.commit`, and `.rollback` throws it away. In production the SQL console is read-only unless you add `write:true`.

`tinker:eloquent` needs PHP 8.2+ and Composer (packages install on the first run) and is for people who already think in Eloquent: `User::first()`, `DB::table('users')->where(...)->update([...])`. Passwords there must be **Argon2id**, because that is what this app verifies. Assigning `password` on the `User` model does it for you, or call `rust_password('secret')`. Details are in `modules/rust-eloquent-tinker/README.md`.

## Running it in production

Laravel needs one cron line (`* * * * * php artisan schedule:run`). Here `schedule:work` is a long-running process, so run it under a supervisor (systemd, Supervisor, a container) and restart it on failure:

```ini
# /etc/systemd/system/rust-starter-scheduler.service
[Service]
ExecStart=/srv/app/rust-starter-cli scheduler
Restart=always
Environment=LOCO_ENV=production
```

Run exactly **one** scheduler per environment, or every job runs once per instance.

## Things to know

- Each job starts a **new process** (`rust-starter-cli task <signature>`). A command cannot see the web server's in-memory cache. If a command must clear a cache the site uses, configure Redis for the cache so they share it.
- Jobs may overlap if one runs longer than its interval. Keep commands short or lock with a database row.
- Test commands like any code: `run_task::<App>(&ctx, Some(&name), &vars)` (see `tests/tasks/pages_check_translations.rs`).
- Heavy work triggered by users (sending email, resizing images) belongs in a background **worker**, not the scheduler.

<!-- foot:start -->
---
[Previous: 12 Cheat sheet and what to build next](12-cheatsheet.md) | [Index](00-index.md) | [Next: 14 Login, register and sessions (`auth` module)](14-auth-and-sessions.md)
<!-- foot:end -->
