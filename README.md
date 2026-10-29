# Rust Starter

Laravel-style starter kit on Loco (Axum), SeaORM and Tera, with MariaDB, Tailwind and HTMX.
The site is bilingual: English and Bangla.

## Run it

```sh
cp .env.example .env            # MariaDB URL and R2 keys
mysql -uroot -e "CREATE DATABASE rust_starter CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
mysql -uroot -e "CREATE DATABASE rust_starter_test CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
cargo run -- db migrate
cargo run -- db seed            # about, privacy-policy, terms-of-service in en + bn
just css                        # needs ./tools/tailwindcss (Tailwind standalone CLI v3.4)
cargo run -- start              # http://localhost:5150
```

Pages: `/`, `/pages/about`, `/pages/privacy-policy`, `/pages/terms-of-service`.
Language: `?lang=bn`, the `lang` cookie, `Accept-Language`, or `/language/bn?next=/path`.

## Modules (`cargo modular`)

Features live in `modules/<name>` crates, like `app-modules/` in Laravel.

```sh
cargo modular make:module blog      # creates modules/blog and registers it
cargo modular modules:list
cargo modular modules:sync          # rebuild packages/modules-registry
```

## Console and schedule

```sh
cargo modular make:command CheckTranslations --module=pages   # creates and registers a Loco task
cargo modular command:list
cargo modular command:run pages:check-translations strict:true
cargo modular schedule:list                                    # cron jobs declared by modules and config/*.yaml
cargo modular schedule:work                                    # run the scheduler (one instance per environment)
```

A module declares jobs with `fn schedule(&self) -> Vec<ScheduledJob>`, for example
`ScheduledJob::command("pages:check-translations").daily_at(3, 0)`.

A module implements `loco_modular::Module`: routes, migrations, Tera templates, tasks, workers, seeder.
The app, the migrator and the view engine all read `modules_registry::all()`.

| Module | Purpose |
|---|---|
| `site-layout` | Public layout, home page, language selection (`Locale` extractor) |
| `pages` | Bilingual static pages (`pages`, `page_translations`) with seeder |
| `auth` | Login, register, logout, DB sessions, CSRF origin check, login lockout |
| `role-permission` | Six-role hierarchy and permissions defined in code (developer holds all) |
| `admin-layout` | Admin shell: permission-filtered sidebar, Ctrl/Cmd+K search, dashboard, users, roles, schedule |
| `media` | Spatie-style media library: single and multiple collections, `HasMedia` |
| `accounts` | Profile, change password, logo upload, inside an account sub-layout |

## Cache and sessions

- Short-lived: `loco_modular::remember(&ctx, key, ttl, async { query }).await`, the `Cache::remember` equivalent (in-memory cache; switch to Redis in config).
- Long-lived: the `lang` cookie (one year) keeps the language; the `session` cookie (30 days, database-backed) keeps you logged in.

Demo login (development only, after `db seed`): `demo@gmail.com` / `secret123$`, a developer who can open `/admin`.

## Tutorials

Start at [`tutorials/00-index.md`](tutorials/00-index.md).

## Tests

```sh
cargo test --workspace
```

Tests run against `rust_starter_test` only. Booting the test environment against any database whose
name does not end in `_test` is refused, because the test environment truncates and recreates its schema.

## MariaDB notes

Loco's `db::seed` YAML helper supports only Postgres and SQLite, so use `loco_modular::seed_yaml`.
