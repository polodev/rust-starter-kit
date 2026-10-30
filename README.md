# Rust Starter

Laravel-style starter kit on Loco (Axum), SeaORM and Tera, with MariaDB, Tailwind and HTMX.
The site is bilingual: English and Bangla.

## What is integrated

Checked means built, tested and working in this repository. Unchecked is planned.

**Foundation**
- [x] Loco app (Axum, SeaORM, Tera) on MariaDB/MySQL, with a separate test database and a guard against testing the real one
- [x] `cargo modular` generators: `make:module`, `make:command`, `modules:list`, `modules:sync`, schedule commands
- [x] Modular monolith: every feature is a `modules/*` crate with its own routes, migrations, templates, permissions and commands
- [x] Tailwind CSS (standalone CLI), Alpine.js, dark and light theme
- [x] English and Bangla interface (Fluent) and bilingual database content with automatic English fallback
- [x] Scheduler and console commands, test suite, 25 tutorials, a conventions guide for people and coding agents (`AGENTS.md`, `CLAUDE.md`, `docs/CONVENTIONS.md`)

**Accounts and access**
- [x] Register, log in, log out with database sessions, CSRF origin check and login lockout
- [x] Six-role hierarchy and `view`/`manage` permissions defined in code, synced to the database
- [x] Sign in with Google for the website and the API, with linked accounts
- [x] Bearer-token JSON API under `/api/v1` with Laravel-shaped errors and an `ApiAuth` extractor
- [x] Profile, change password and profile image for the signed-in person
- [ ] Email verification, password reset, two-factor sign-in
- [ ] Apple sign-in and mobile OTP login

**Admin**
- [x] Admin shell: permission-filtered sidebar, navigation hub, Ctrl/Cmd+K search, breadcrumbs, one wide container
- [x] Users: list, show, edit (name, email, profile image) and generated password
- [x] Roles and permissions: overview, roles, permissions, matrix, user assignments, backend links, sync
- [x] Pages: list, show, bilingual editor (English required, Bangla optional), feature image, publish or hide
- [x] Media library page with lightbox, direct upload, search and filters
- [x] Contacts inbox: list, modal and a replied status
- [x] Scheduled jobs page
- [ ] Settings, menus and blog
- [x] Activity log (Spatie-compatible table): filters, before/after changes, delete and clean; entries written by the admin screens and sign-in
- [x] Queue monitor: tracked job history (queued, running, finished, failed, stale), pending and failed jobs of a Laravel queue, delete, purge, mark stale

**Content and files**
- [x] Media library compatible with Spatie laravel-medialibrary: collections, `HasMedia`, same `media` table and file layout, `media:import`
- [x] Public files redirect to a CDN or S3 URL when configured; private files are access-checked
- [ ] Image conversions (thumbnails) and responsive images
- [ ] Sitemap, SEO tags and `robots.txt`

**Public site**
- [x] Bilingual pages (About, Privacy Policy, Terms of Service) with a language switch
- [x] Contact form that submits without a page reload, with a honeypot, rate limit and origin check
- [ ] CAPTCHA option for the contact form

**Developer tools**
- [x] `tinker`: SQL console with transactions, a confirm step and read-only mode in production
- [x] PDF generation with headless Chrome (Browsershot-style), Bangla included, with a sample invoice
- [x] Email through Amazon SES (SMTP) or any SMTP server, HTML and text, PDF attachments, a `mail_log` of every send, and `cargo modular make:mail`
- [x] AI service: one wrapper for Claude, OpenAI and Gemini (text, pictures to JSON) and OpenAI image generation, default provider and models from `.env`, and AI API logs with bulk delete
- [x] Database backups: `backup:run only-db:true` to local, S3 or R2 disks configured separately from the site storage, retention, monitor and restore
- [x] `tinker:eloquent`: Eloquent models and the `DB` facade on the same database (PHP needed)
- [x] Database shared with a Laravel app: Spatie `media`, Argon2id passwords, binary `pid`
- [ ] OpenAPI documentation for the API
- [ ] Payments, tenant URLs and a project generator (`cargo modular new`)

## Run it

```sh
cp .env.example .env            # database URLs, R2 keys, backup settings (read automatically at startup)
mysql -uroot -e "CREATE DATABASE rust_starter CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
mysql -uroot -e "CREATE DATABASE rust_starter_test CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
cargo run -- db migrate
cargo run -- db seed            # about, privacy-policy, terms-of-service in en + bn
just css                        # needs ./tools/tailwindcss (Tailwind standalone CLI v3.4)
cargo run -- start              # http://localhost:5150
```

Start over at any time with `cargo run -- db reset` then `cargo run -- db seed` (each table has one base migration).

Pages: `/`, `/pages/about`, `/pages/privacy-policy`, `/pages/terms-of-service`.
Language: `?lang=bn`, the `lang` cookie, `Accept-Language`, or `/language/bn?next=/path`.

## Modules (`cargo modular`)

Features live in `modules/<name>` crates, like `app-modules/` in Laravel. Shared crates live in `packages/`: `loco-modular` (the `Module` trait), `modules-registry` (generated list of modules), `cargo-modular` (the CLI) and `loco-activity-log` (the activity-log writer, usable by any module).

```sh
cargo modular make:module blog      # creates modules/blog and registers it
cargo modular make:mail InvoiceReady --module=blog   # mail class + HTML and text templates (like Laravel's make:mail)
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

Commands that ship with the kit (run as `cargo run -- task <name> key:value`):

| Command | What it does | Scheduled |
|---|---|---|
| `backup:run only-db:true` (`disk:`, `filename:`) | dump the database (gzip) to the backup bucket(s) | daily 01:30 |
| `backup:list`, `backup:clean`, `backup:monitor` | list, apply retention, check there is a recent backup | clean 01:00, monitor 03:00 |
| `activitylog:clean days:90 log:users` | delete old activity-log entries | daily 03:20 |
| `queue-watcher:mark-stale`, `queue-watcher:purge` | flag hung jobs, remove old job history | hourly, daily 03:40 |
| `auth:prune-sessions`, `auth-api:prune-tokens` | remove expired sessions and API tokens | hourly, daily 03:30 |
| `pages:check-translations` | report pages that lack their English text | daily 03:00 |
| `mail:send-test`, `mail:send-invoice` (`to:you@example.com`) | send a test email, or a sample invoice PDF made with Chrome and emailed | |
| `mail:ses-smtp-password` | print the SES SMTP user and the password derived from your IAM key (for Loco's own mailer) | |
| `ai-service:status`, `ai-service:ask`, `ai-service:generate-image` | show the AI provider settings, ask a question, make a picture | |
| `ai-service:purge-ai-api-logs days:30` | delete old AI API logs | daily 03:50 |
| `pdf:sample out:/tmp/invoice.pdf` | write a sample invoice PDF | |
| `roles:sync` | copy roles and permissions from code into the database | |
| `media:move-local-uploads` (`dry-run:true`) | copy files from `storage/uploads` to the R2 public bucket (`MEDIA_STORAGE=r2`) and update their rows | |
| `media:move-local-uploads` (`dry-run:true`) | copy files from `storage/uploads` to the R2 public bucket (`MEDIA_STORAGE=r2`) and update their rows | |
| `media:import database:name` | copy a Laravel (Spatie) `media` table into this database | |
| `tinker`, `tinker:eloquent` | SQL console, Eloquent and `DB` facade console | |
| `user:create`, `user:delete` | manage users | |

### Tinker (change the database from a prompt)

Two consoles, both started as tasks:

```sh
cargo run -- task tinker            # SQL console: select * from users limit 5;  .tables  .desc users
cargo run -- task tinker:eloquent   # Laravel-style: User::first(), DB::table('users')->update([...])
```

- `tinker` is built in (no extra tools). It takes SQL ending with `;`, has `.tables`, `.desc <table>`,
  `.count <table>`, `.hash <password>`, and `.begin` / `.commit` / `.rollback` so you can try a change first.
  `UPDATE` or `DELETE` without `WHERE` asks for confirmation. In production it is read-only unless you
  add `write:true`.
- `tinker:eloquent` is the `rust-eloquent-tinker` module: PsySH, Eloquent models and the `DB` facade
  pointed at this app's database, for people who already know Laravel tinker. It needs PHP 8.2+ and
  Composer (packages install on first run). Passwords must be Argon2id (`rust_password('x')`), not
  bcrypt. See `modules/rust-eloquent-tinker/README.md`.

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
| `media` | Spatie-compatible media library (same `media` table and `{id}/{file}` layout): collections, `HasMedia`, `media:import` |
| `accounts` | Profile, change password, logo upload; admin user show/edit (name, email, image, generated password) |
| `auth-api` | Sanctum-style bearer tokens under `/api/v1/auth`, `ApiAuth` extractor for other API modules |
| `social-auth` | Sign in with Google for the website and the API (ID token), linked accounts |
| `contact` | Public `/contact` form that submits without a reload, with a honeypot; admin inbox with a modal and a replied status |
| `activity-log` | Spatie-compatible activity log: admin list and detail, `activitylog:clean`; writing entries uses the `loco-activity-log` package |
| `queue-watcher` | Queue monitor: tracked job history, pending and failed jobs, delete, purge; `track()` for workers |
| `db-backup` | `backup:run only-db:true`, list, clean, monitor; dumps to its own S3/R2/local disks |
| `pdf` | PDF generation with headless Chrome (like Browsershot); sample invoice |
| `ai-service` | One wrapper for Claude, OpenAI and Gemini (text, pictures to JSON) and OpenAI images; AI API logs at `/admin/ai-api-logs` |
| `mail` | Email via Amazon SES or SMTP: templates, attachments, `mail_log`, `mail:send-invoice` |
| `rust-eloquent-tinker` | Dev console with Eloquent and the `DB` facade on this database (`task tinker:eloquent`) |

## Cache and sessions

- Short-lived: `loco_modular::remember(&ctx, key, ttl, async { query }).await`, the `Cache::remember` equivalent (in-memory cache; switch to Redis in config).
- Long-lived: the `lang` cookie (one year) keeps the language; the `session` cookie (30 days, database-backed) keeps you logged in.

Demo login (development only, after `db seed`): `demo@gmail.com` / `secret123$`, a developer who can open `/admin`.

## Inspired by

This starter pack borrows good ideas from established, public projects and keeps their vocabulary where Rust allows it:

| Idea | From | Here |
|---|---|---|
| Convention over configuration, generators | Ruby on Rails, [Loco](https://loco.rs) | the framework underneath |
| Modular monolith (`app-modules`) | [internachi/modular](https://github.com/InterNACHI/modular) (Laravel) | `modules/*` crates, `cargo modular` |
| `artisan`, scheduler, `tinker` | Laravel | `cargo modular`, Loco tasks, `task tinker` |
| Eloquent models and the `DB` facade | Laravel | SeaORM; real Eloquent in `task tinker:eloquent` |
| Token API and Google sign-in | Laravel Sanctum and Socialite | `auth-api`, `social-auth` |
| Update a form without a page load | Laravel Livewire and Volt | server-rendered "components" (`contact`) |
| Activity log | [Spatie laravel-activitylog](https://spatie.be/docs/laravel-activitylog) | `activity-log` (same table), `loco-activity-log` |
| Database backups (`backup:run --only-db`) | [Spatie laravel-backup](https://spatie.be/docs/laravel-backup) | `db-backup` (database only, own bucket, same retention strategy) |
| Queue monitoring | Laravel queue monitors (Horizon-style) | `queue-watcher` |
| PDF generation | [Spatie Browsershot](https://spatie.be/docs/browsershot) and laravel-pdf | `pdf` (headless Chrome) |
| Mailables and `make:mail` | Laravel Mail | `mail`, `cargo modular make:mail` |
| Media collections, `HasMedia` | [Spatie laravel-medialibrary](https://spatie.be/docs/laravel-medialibrary) | `media` (same table and file layout) |
| Roles and permissions | Laravel gates and policies, Spatie permission | `role-permission` |
| Fluent translations | Mozilla Fluent | `assets/i18n` |
| Styling and small interactions | Tailwind CSS, Alpine.js | admin and site templates |

## Conventions

How modules, routes, permissions, admin screens, translations, the API, the database and tests are done in
this project is written down in [`docs/CONVENTIONS.md`](docs/CONVENTIONS.md). Read it before changing code;
it is also what `AGENTS.md` and `CLAUDE.md` point coding agents to.

## Tutorials

Start at [`tutorials/00-index.md`](tutorials/00-index.md).

## Tests

```sh
cargo test --workspace
```

Run one `cargo test` at a time: the tests share one test database. `tests/migrations.rs` checks that every table has a
single base migration. The database-backup test needs `mysqldump` and the invoice test needs Chrome; each is skipped without it. Tests run against `rust_starter_test` only. Booting the test environment against any database whose
name does not end in `_test` is refused, because the test environment truncates and recreates its schema.

## MariaDB notes

Loco's `db::seed` YAML helper supports only Postgres and SQLite, so use `loco_modular::seed_yaml`.
