<!-- nav:start -->
**Tutorial 28 of 28** | [Index](00-index.md) | [Previous: 27 PostgreSQL for MySQL users](27-postgresql-for-mysql-users.md) | Next: none

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
26. [Commands and jobs for Laravel developers](26-commands-and-jobs-for-laravel-developers.md)  
27. [PostgreSQL for MySQL users](27-postgresql-for-mysql-users.md)  
**28. [MySQL or PostgreSQL, one choice per app](28-mysql-or-postgresql-per-app.md)**  

</details>
<!-- nav:end -->

# 28. MySQL or PostgreSQL, one choice per app

Some of your apps will run on MariaDB/MySQL and some on PostgreSQL. This starter supports both from one codebase. The
database is chosen **when you build the app**, and each app uses one of them.

## Why at build time

Laravel creates MySQL ids as `bigint unsigned`. A signed Rust integer cannot read those, so the entities must use unsigned
types on MySQL. PostgreSQL has no unsigned integers at all, and SeaORM refuses unsigned Rust types there. One compiled
program cannot be both, so a cargo feature flips the integer types:

| | MySQL / MariaDB (default) | PostgreSQL (`--features postgres`) |
|---|---|---|
| ids | `u64` (`bigint unsigned`) | `i64` (`bigint`) |
| unix times, sizes | `u32` (`int unsigned`) | `i32` (`integer`) |
| small counters | `u16`, `u8` | `i32`, `i16` |

The PostgreSQL types are exactly what Laravel creates on PostgreSQL, so a Laravel dump of either kind imports as is
(`jobs`, `failed_jobs`, `media`, `activity_log`).

## Start an app on MySQL (the default)

```sh
cp .env.example .env     # DATABASE_URL=mysql://root@127.0.0.1:3306/my_app
mysql -uroot -e "CREATE DATABASE my_app CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
cargo run -- db migrate
cargo run -- start --server-and-worker
```

## Start an app on PostgreSQL

```sh
createdb my_app
# .env: DATABASE_URL=postgres://postgres:secret@127.0.0.1:5432/my_app
cargo run --features postgres -- db migrate
cargo run --features postgres -- start --server-and-worker
```

Every cargo command for that app carries `--features postgres`. To avoid typing it, put it in the app's `justfile`, or set
`default = ["postgres"]` in the root `Cargo.toml` `[features]` of an app that is always PostgreSQL.

If the build and the URL disagree the app stops at start-up with a message that names the fix, for example:
`this build is for PostgreSQL but DATABASE_URL is a MySQL/MariaDB database: use a postgres:// DATABASE_URL, or build without --features postgres`.

## Rules for code that works on both

1. **Entities:** an unsigned column uses `loco_database::UnsignedBigInt` / `UnsignedInt` / `UnsignedSmallInt` /
   `UnsignedTinyInt`, never `u64`/`u32`. Convert with `big_int(7)`, `int(30)`, `to_u64(row.id)`.
2. **Ids from the URL:** `Path(id): Path<UnsignedBigInt>`.
3. **Migrations:** the portable way is SeaORM's schema builder (`Table::create()...`), which makes the right SQL for each
   database. If you need raw SQL (to match a Laravel table exactly), give both versions:

   ```rust
   loco_database::run_ddl(m, CREATE_THINGS_MYSQL, CREATE_THINGS_POSTGRES).await
   ```

   `CREATE_THINGS_POSTGRES` is a list of statements: `bigserial` for the id, `timestamptz`, `text`, and each index as its
   own `CREATE INDEX IF NOT EXISTS` with a name that is unique in the whole schema.
4. **No MySQL-only SQL** in code: `SHOW TABLES`, backticks, `ON DUPLICATE KEY`. Use SeaORM, or branch on
   `ctx.db.get_database_backend()` (the `tinker` console does this for `.tables` and `.desc`).
5. **Case:** `LIKE` ignores case on MySQL and respects it on PostgreSQL. Search boxes use `loco_database::contains_ignoring_case(Column::Name, text)`, which uses `ILIKE` on a PostgreSQL build.
6. **Booleans:** write `true`/`false`, not `1`/`0`, in SQL; PostgreSQL has a real boolean type.

## Test on both before you ship

```sh
cargo test --workspace                                              # MySQL / MariaDB

createdb my_app_test
TEST_DATABASE_URL=postgres://user@127.0.0.1:5432/my_app_test \
  TEST_POSTGRES_URL=postgres://user@127.0.0.1:5432/postgres \
  cargo test --workspace --features loco-database/postgres --target-dir target-pg
```

Use the separate `--target-dir` so switching databases does not recompile everything. The MariaDB-only backup test
skips itself on a PostgreSQL build; `TEST_POSTGRES_URL` turns on the real `pg_dump` and restore test.

## Backups, viewing data and queues

- **Backups** (`backup:run`): the database address decides the tool, `mysqldump` for `mysql://`, `pg_dump` for `postgres://`,
  like Spatie's `mysql` and `pgsql` dumpers (tutorial 22).
- **Viewing data:** Adminer opens both: <http://adminer.test> (PostgreSQL) and <http://adminer.test/?mariadb>. See tutorial 27 for
  `psql` next to `mysql`.
- **Queues:** the database queue (`jobs`, `failed_jobs`, tutorial 26) works on both. Mail's `mail_queue` too.

## Where the code is

`packages/loco-database` (aliases, `run_ddl`, start-up check), the PostgreSQL SQL next to the MySQL SQL in each raw
migration, and the tests in `tests/`.

<!-- foot:start -->
---
[Previous: 27 PostgreSQL for MySQL users](27-postgresql-for-mysql-users.md) | [Index](00-index.md) | Next: none
<!-- foot:end -->
