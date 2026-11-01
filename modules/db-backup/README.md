# db-backup

Database backups, like `php artisan backup:run --only-db` in Spatie laravel-backup, but **database only** (no
files). A dump is gzipped and stored on one or more disks that are configured **separately from the website's
storage**, so backups can go to another bucket on another account.

```sh
cargo run -- task backup:run only-db:true        # dump and store on every destination
cargo run -- task backup:run disk:r2 filename:before-upgrade
cargo run -- task backup:list
cargo run -- task backup:clean dry-run:true      # preview what retention would delete
cargo run -- task backup:monitor                 # non-zero exit when a destination has no recent backup
```

`only-db:true` is accepted for familiarity; this module only ever backs up the database.

## Quick start (what this repository ships)

`config/development.yaml` and `config/production.yaml` already contain a `db_backup` block that reads its values from
the environment, so you only fill `.env` (see `.env.example`):

```sh
DB_BACKUP_ENABLED=true
BACKUP_NAME=my-app
BACKUP_BUCKET=my-private-bucket
BACKUP_PREFIX=db-backups
BACKUP_ENDPOINT=https://<account>.r2.cloudflarestorage.com
BACKUP_ACCESS_KEY_ID=...
BACKUP_SECRET_ACCESS_KEY=...
```

With `DB_BACKUP_ENABLED` unset or `false` the block is switched off: nothing is checked, scheduled runs are skipped
and a manual `backup:run` says it is not configured. The `BACKUP_*` keys are separate from the site's `R2_*` keys, so
backups can use another bucket or another account. Use a **private** bucket (never one that serves files publicly).

## Settings (`config/<environment>.yaml`)

```yaml
settings:
  db_backup:
    enabled: true                      # false switches it off (the rest is then not checked)
    name: my-app                       # folder on every disk: {name}/2026-10-30-01-30-00.sql.gz
    destinations: [local, r2]          # where backup:run stores the dump
    disks:
      local:
        driver: local
        path: storage/backups
      r2:
        driver: s3                     # AWS S3, Cloudflare R2, MinIO, Backblaze B2 ...
        bucket: my-backups
        region: auto
        endpoint: https://<account>.r2.cloudflarestorage.com   # omit for AWS S3
        access_key_id: <%= get_env(name="BACKUP_KEY", default="") %>
        secret_access_key: <%= get_env(name="BACKUP_SECRET", default="") %>
        prefix: db                     # optional folder inside the bucket
    retention:                         # Spatie's default strategy (these are the defaults)
      keep_all_days: 7
      keep_daily_days: 16
      keep_weekly_weeks: 8
      keep_monthly_months: 4
      keep_yearly_years: 2
    monitor_max_age_days: 1
    # database_uri: postgres://user:pass@127.0.0.1:5432/app   # back up this database instead of the app's own
    # dump_binary: /usr/local/bin/mysqldump     # mysqldump, mariadb-dump or pg_dump, when it is not on the PATH
    # extra_dump_args: ["--no-tablespaces"]
```

Keep keys out of the file: use `get_env` as above. These disks are independent of the app's `storage:` setting.

## How it works

- The database is the app's own (`database.uri`) unless `settings.db_backup.database_uri` names another one. The
  address decides the tool, like Spatie's `mysql` and `pgsql` dumpers: `mysql://` or `mariadb://` runs `mysqldump`,
  `postgres://` or `postgresql://` runs `pg_dump`. The password is passed in the environment (`MYSQL_PWD`,
  `PGPASSWORD`), never on the command line.
- A PostgreSQL dump is plain SQL made with `pg_dump --no-owner --no-password`, so any user can restore it.
- The MySQL dump uses `--single-transaction --quick --skip-lock-tables --routines --triggers` and utf8mb4. A MySQL 8
  client gets `--column-statistics=0` automatically so it can dump a MariaDB server.
- The dump is streamed through gzip into a temporary file and uploaded in chunks, so large databases are not
  held in memory. The same file goes to every destination; if one disk fails the others still get it, and the
  command reports the failure.
- **Retention** (`backup:clean`) keeps everything for `keep_all_days`, then the newest backup of each day, week,
  month and year in their ranges. The newest backup is never deleted.
- **Scheduler:** `backup:clean` at 01:00, `backup:run` at 01:30 and `backup:monitor` at 03:00 daily. They say so and
  do nothing until `settings.db_backup` exists.

## Restore

```sh
gunzip -c my-app/2026-10-30-01-30-00.sql.gz | mysql -u root my_database          # MySQL, MariaDB
createdb my_database && gunzip -c my-app/2026-10-30-01-30-00.sql.gz | psql my_database   # PostgreSQL
```

## Needs

An S3 disk needs no extra setup (an HTTP client is installed for it automatically). `mysqldump` (MySQL or MariaDB client tools) or `pg_dump` (PostgreSQL client tools; with Homebrew it is under `/usr/local/opt/postgresql@16/bin`, which the module also tries) on the machine that runs the command. One command backs up one database; to back up a second database, run another configuration with its own `name`.

## Not included

Backing up files, email or chat notifications, and a size limit for deleting the oldest backups.
