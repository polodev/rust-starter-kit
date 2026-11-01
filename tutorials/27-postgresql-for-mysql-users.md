<!-- nav:start -->
**Tutorial 27 of 28** | [Index](00-index.md) | [Previous: 26 Commands and jobs for Laravel developers](26-commands-and-jobs-for-laravel-developers.md) | [Next: 28 MySQL or PostgreSQL, one choice per app](28-mysql-or-postgresql-per-app.md)

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
**27. [PostgreSQL for MySQL users](27-postgresql-for-mysql-users.md)**  
28. [MySQL or PostgreSQL, one choice per app](28-mysql-or-postgresql-per-app.md)  

</details>
<!-- nav:end -->

# 27. PostgreSQL in the terminal, for MySQL users

You work with `mysql -uroot` and `mysqldump`. PostgreSQL has the same tools under other names. In this project
the website's data stays in MariaDB and the background job queue is in Redis (tutorial 26). PostgreSQL is installed for
learning and optional use; the app does not need it.

## Set up the commands once

Homebrew installs PostgreSQL 16 without putting its tools on your PATH. Add this line to `~/.zshrc`, then open a new terminal:

```sh
export PATH=/usr/local/opt/postgresql@16/bin:$PATH
```

```sh
brew services start postgresql@16      # start it (starts again at login)
brew services stop postgresql@16
pg_isready                             # is it running?
```

## The two worlds side by side

| MySQL / MariaDB | PostgreSQL | Note |
|---|---|---|
| `mysql -uroot` | `psql -d postgres` | your Mac user (`polodev`) is the superuser, no password |
| `mysql -uroot rust_starter` | `psql rust_starter` | connect to one database |
| `mysql -u root -p -h 127.0.0.1 db` | `psql -h 127.0.0.1 -U polodev rust_starter` | or `psql postgres://polodev@127.0.0.1:5432/rust_starter` |
| `SHOW DATABASES;` | `\l` | list databases |
| `USE db;` | `\c db` | switch database inside `psql` |
| `SHOW TABLES;` | `\dt` | list tables |
| `DESCRIBE users;` | `\d users` | columns, indexes, keys |
| `SHOW CREATE TABLE users;` | `\d+ users` (or `pg_dump -s -t users`) | |
| `SELECT DATABASE();` | `SELECT current_database();` | |
| `CREATE DATABASE db;` | `createdb db` or `CREATE DATABASE db;` | |
| `DROP DATABASE db;` | `dropdb db` or `DROP DATABASE db;` | |
| `exit` | `\q` | |
| `` `column` `` quotes | `"column"` quotes | strings use single quotes `'text'` in both |
| `AUTO_INCREMENT` | `GENERATED ALWAYS AS IDENTITY` / `serial` | |
| `LIMIT 10 OFFSET 5` | same | |

Useful inside `psql`: `\x` (vertical output, like `\G`), `\timing` (show query time), `\?` (all commands), `\i file.sql` (run a file).

## Export (like `mysqldump`)

```sh
pg_dump rust_starter > rust_starter.sql                  # mysqldump rust_starter > rust_starter.sql
pg_dump -s rust_starter > schema.sql                     # structure only   (mysqldump --no-data)
pg_dump -a rust_starter > data.sql                       # data only        (mysqldump --no-create-info)
pg_dump -t pg_loco_queue rust_starter > one_table.sql    # one table
pg_dump rust_starter | gzip > rust_starter.sql.gz        # compressed
pg_dump -Fc rust_starter -f rust_starter.dump            # "custom" format: compressed, restores selectively
pg_dumpall > everything.sql                              # every database and the users (needs the superuser)
```

## Import (like `mysql db < file.sql`)

```sh
createdb rust_starter_copy                               # unlike mysqldump --databases, the dump does not create the database
psql rust_starter_copy < rust_starter.sql                # mysql rust_starter_copy < rust_starter.sql
psql -q -v ON_ERROR_STOP=1 -f rust_starter.sql rust_starter_copy    # stop at the first error
gunzip -c rust_starter.sql.gz | psql rust_starter_copy
pg_restore -d rust_starter_copy rust_starter.dump        # for the -Fc format
pg_restore -l rust_starter.dump                          # list what is inside
pg_restore -d rust_starter_copy -t pg_loco_queue rust_starter.dump   # restore one table
```

`pg_dump` plain files are text SQL, so you can read them, and `psql` runs them. A `.dump` file made with `-Fc` is binary and only
`pg_restore` reads it.

## Copy a database in one line

```sh
createdb -T rust_starter rust_starter_backup             # instant copy, nobody may be connected to the source
```

## Remote servers

```sh
pg_dump -h db.example.com -U app_user app_db > app_db.sql
PGPASSWORD=... pg_dump ...            # avoids the prompt; better: a ~/.pgpass file (chmod 600) with host:port:db:user:password
```

Never put real passwords in a script that is committed to git.

## Users and permissions

```sql
CREATE USER app_user WITH PASSWORD 'change-me';          -- CREATE USER 'app_user'@'%' IDENTIFIED BY ...
CREATE DATABASE app_queue OWNER app_user;
GRANT ALL PRIVILEGES ON DATABASE app_queue TO app_user;
\du                                                      -- SHOW GRANTS / list users
```

## This project

| What | Command |
|---|---|
| The website database (MariaDB) | `mysql -uroot rust_starter` and `mysqldump -uroot rust_starter > site.sql` as before |
| Website backups | `cargo run -- task backup:run only-db:true` (the `db-backup` module, MariaDB) |
| A browser view | <http://adminer.test> (Adminer in `~/sites/adminer`, Herd) |

The job queue connection is `QUEUE_REDIS_URL` in `.env` (Redis, not PostgreSQL).

## Differences that surprise MySQL users

- Names are lower-case unless quoted. Use `snake_case` and you never notice.
- A failed statement inside a transaction aborts the whole transaction until `ROLLBACK`.
- `GROUP BY` is strict: every selected column must be grouped or aggregated.
- There are no unsigned integers, and `LIKE` is case-sensitive (use `ILIKE`).
- `VACUUM` is PostgreSQL's housekeeping. Autovacuum does it for you; you rarely run it by hand.
- Connections are heavier than in MySQL, so many app processes need a pooler (PgBouncer) at larger scale.

<!-- foot:start -->
---
[Previous: 26 Commands and jobs for Laravel developers](26-commands-and-jobs-for-laravel-developers.md) | [Index](00-index.md) | [Next: 28 MySQL or PostgreSQL, one choice per app](28-mysql-or-postgresql-per-app.md)
<!-- foot:end -->
