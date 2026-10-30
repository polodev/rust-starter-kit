# Conventions of this starter pack

How code in this repository is organised and written. Follow these rules when you add or change
anything; they are what the existing modules already do. When a rule and the code disagree, fix the
code or update this file in the same change.

This is a [Loco](https://loco.rs) app (Rust, "Rails for Rust") written by someone who also builds Laravel
apps, so the vocabulary is borrowed from Laravel where Rust allows it. The database is MariaDB/MySQL and is
meant to stay compatible with Laravel apps (see [Database](#database)).

## Contents

1. [The workflow for any change](#the-workflow-for-any-change)
2. [Names: long, full and specific](#names-long-full-and-specific)
3. [Modules](#modules)
4. [Routes and URLs](#routes-and-urls)
5. [Controllers and models](#controllers-and-models)
6. [Permissions and roles](#permissions-and-roles)
7. [Admin screens](#admin-screens)
8. [Public pages, templates and translations](#public-pages-templates-and-translations)
9. [Forms, components and modals](#forms-components-and-modals)
10. [JSON API](#json-api)
11. [Database](#database)
12. [Media library](#media-library)
13. [Security](#security)
14. [Console commands and the scheduler](#console-commands-and-the-scheduler)
15. [Tests](#tests)
16. [Documentation and git](#documentation-and-git)
17. [Checklists](#checklists)

## The workflow for any change

1. Generate, then edit: `cargo modular make:module`, `make:command`, or Loco's `cargo loco generate`. Rust has
   no autoloading, so hand-wiring is how "the handler exists but returns 404" happens.
2. Use what the app already has (ORM, cache, mailer, storage, scheduler, tasks, test harness) before adding a
   crate. A new dependency needs a reason that you can state in one sentence.
3. Write the test with the feature, not afterwards.
4. Before you call it done, run all three, in this order, one at a time:

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

Never run two `cargo test` commands at once: the tests share one test database and will collide. Run the
suite and check the result before starting another run.

## Names: long, full and specific

As the project grows, short or general names make people unsure what a thing is. Name everything by exactly what
it is, in full. This applies to code, routes, screens, permissions, tables, commands, templates and docs.

- **Say the subject.** The history of calls to AI providers is "AI API logs" (`ai_api_logs`, `/admin/ai-api-logs`,
  `ai_api_logs.view`), never "API logs" or just "logs". Other logs will arrive (error logs, activity logs, mail logs);
  each keeps its own full name: `activity_logs.view`, not `logs.view`.
- **No abbreviations or one-word stand-ins** for things with a longer name: `ai_api_log_row`, `AiApiLogFilters`,
  `ai_api_logs_delete_selected`, not `row`, `Filters`, `delete`. Short names are fine only where the framework fixes
  them (`ctx`, `v` for the view engine, `q` for the search parameter).
- **Relations use the full name** of what they relate: `permission_roles`, `page_translations`, never `pr` or `trans`.
- **Commands** say what they act on: `ai-service:purge-ai-api-logs`, not `ai:purge`.
- **Screens** carry the subject in the menu label, the breadcrumb and the page title: "AI API logs", not "Logs".
- **Templates** are `<subject>_<screen>.html`: `ai_api_logs_index.html`, `ai_api_log_show.html`.
- **Permissions** are `<full_area>.view` and `<full_area>.manage`, one area per kind of thing.

## Modules

The app is a modular monolith in the spirit of `internachi/modular`. Each feature is a crate in `modules/`.

```text
modules/<name>/
  Cargo.toml            crate `module-<name>`
  README.md             what it does, routes, settings (keep it current)
  src/lib.rs            `<Name>Module` implementing `loco_modular::Module`
  src/controllers/      handlers and `routes()`
  src/entities/         SeaORM entities
  src/migrations/       this module's migrations, registered in `mod.rs`
  src/commands/         console commands (Loco tasks)
  templates/            Tera templates, registered in `templates()`
```

- Create one with `cargo modular make:module <kebab-name>`. It wires `Cargo.toml`, the module registry
  (`packages/modules-registry`) and the workspace for you. Run `cargo modular modules:sync` if the registry drifts.
- Names: directory `kebab-case`, crate `module-kebab-case`, struct `PascalCaseModule`.
- A module owns its tables, routes, templates, permissions and commands. Another module reaches into it only
  through its `pub use` items.
- **Where code lives:** put admin screens for an entity in the module that owns the entity (`accounts` owns the
  user screens), and generic admin chrome in `admin-layout`.
- **Dependency direction:** no cycles. `admin-layout` depends on `auth` and `role-permission`; `media` depends on
  `admin-layout`; so `admin-layout` can never depend on `media`. If a screen needs another module, put the screen
  in the module that sits lower in the graph.
- Templates are compiled into the binary with `include_str!`. After changing a template or a `.ftl` file, restart
  the server and rebuild the CSS (`./tools/tailwindcss -c tailwind.config.js -i assets/css/input.css -o assets/static/css/app.css --minify`).

| Module | Owns |
|---|---|
| `site-layout` | public layout, language and theme, `Site` extractor |
| `admin-layout` | admin shell, dashboard, user list, role and permission screens, schedule |
| `auth` | web login, register, sessions, `Auth` / `OptionalAuth` extractors |
| `auth-api` | bearer-token API, `ApiAuth` extractor |
| `social-auth` | sign in with Google (web and API) |
| `role-permission` | roles and permissions defined in code |
| `accounts` | the signed-in person's area and the admin user screens |
| `pages` | bilingual static pages |
| `media` | Spatie-compatible media library |
| `contact` | contact form and admin inbox |
| `activity-log` | admin screens and cleaning for the Spatie-compatible activity log |
| `queue-watcher` | queue monitor: tracked job history, pending and failed jobs |
| `db-backup` | database backups (`backup:run only-db:true`) to separately configured disks |
| `pdf` | PDF generation with headless Chrome |
| `ai-service` | the one wrapper for Claude, OpenAI and Gemini, OpenAI images, and the AI API logs |
| `mail` | sending email (Amazon SES or SMTP), `mail_log`, `make:mail` |
| `rust-eloquent-tinker` | development console (Eloquent and `DB` facade) |

## Routes and URLs

| Kind | Prefix | Example |
|---|---|---|
| Public website | none | `/pages/about`, `/contact` |
| Signed-in person's own area | `/account` | `/account/profile` |
| Admin | `/admin` | `/admin/users`, `/admin/users/{id}/edit` |
| JSON API | `/api/v1` (always starts with `/api`) | `/api/v1/auth/login` |
| Files | `/media/{id}/{file_name}` | |

- Admin resources are RESTful: `GET /admin/things` (list), `GET /admin/things/{id}` (read-only show),
  `GET /admin/things/{id}/edit` (form), `POST /admin/things/{id}` (save), `POST /admin/things/{id}/<action>`
  for other actions. HTML forms can only send GET and POST, so do not use PUT, PATCH or DELETE for them.
- Static segments before `{id}` segments. Use `{id}` for numeric ids and a readable slug only on public pages.
- The old `/api/auth/*` routes come from the Loco starter (JWT). New API work goes under `/api/v1`.

## Controllers and models

- **Fat model, slim controller.** A handler parses input, checks permission, calls a model or service function,
  and renders. Domain rules live on the model or in the module's own service file.
- Handlers use `#[debug_handler]`, take extractors in a fixed order (`ViewEngine`, `State`, `HeaderMap`, the
  `AdminPage`/`Auth` extractor, `Path`, `Query`/`Form`), and return `loco_rs::Result<Response>`.
- Use descriptive names (`reply_status`, not `rs`). A comment explains why, never what the line does.
- Validate on the server, always. Return the form again with the typed values and a message (HTTP 422) rather than
  throwing the input away.
- Reading is cheap, writing needs thought: check `same_origin` and the permission before you touch the database.
- Use SeaORM (`Entity::find().filter(...)`, `ActiveModel` with `Set(..)`). Set `created_at` and `updated_at`
  yourself when the entity does not.

## Permissions and roles

- Roles and permissions are defined in code in `modules/role-permission/src/definitions.rs`, then copied into
  the database by `cargo run -- task roles:sync` (or the Sync button on `/admin/role-permissions`).
- An area has two permissions: `<area>.view` and `<area>.manage` (`pages.view`, `contacts.manage`, `ai_api_logs.view`). Add the area
  to `AREAS`, then grant it to roles in `role_definitions()`. Roles inherit everything below them:
  `new_employee < business_development < marketing < employee < admin < developer`.
- `roles.*` permissions come only from a role, never as a direct exception to a person.
- In a handler: `if !page.can("things.view") { return page.forbidden(&v); }`. Check `view` for reading and
  `manage` for every change, and hide controls the person may not use.
- A new permission changes counts shown on the dashboard and in tests; update them.

## Admin screens

- Every admin page extends `layouts/admin.html` and uses `AdminPage` (it gives the shell data, the sidebar
  and `can()`).
- **One width.** The layout wraps all pages in one container (`max-w-[1920px]`). Do not add your own `max-w-*`
  or `mx-auto` wrapper around a page.
- **Breadcrumb** on every page, in `{% block breadcrumb %}`, simple and always starting at Dashboard:
  `Dashboard › Users › Demo User › Edit`. If you do not override the block the layout shows
  `Dashboard › {{ title }}`. Do not add "back" links; the breadcrumb is the back path.
- **Lists:** the first column is **ID** and it is a link (`data-testid="id-link"`) to the show page. Then the
  useful columns, then a right-aligned Actions column with **Show** and, if permitted, **Edit**. Searchable
  lists use a `q` query parameter; filters use `status` and similar.
- **Show** is read-only and has an Edit button for people with `manage`. **Edit** is a separate page.
- Tables: wrap in a rounded bordered card, `text-sm`, `divide-y` rows, `overflow-x-auto`.
- Feedback uses a query string (`?saved=profile`, `?error=email`) mapped to text in the controller, shown in
  one coloured banner (`data-testid="status"`). Do not put messages in the URL as free text.
- Sidebar entries come from the module's `admin_menu()` (`MenuItem`): section, label, href, permission, icon
  (Font Awesome), order, `match_children`.
- Every interactive element a test needs has a `data-testid`.
- Dark mode: every colour class has a `dark:` partner.

## Public pages, templates and translations

- Public pages extend `layouts/site.html` and use the `Site` extractor; render with `site.data(json!({...}))`.
- Template names: `<module>/<page>.html` for pages, `admin/<name>.html` for admin, a leading underscore for a
  partial (`contact/_form.html`). Register each one in the module's `templates()`.
- The site is bilingual: English (`en-US`) and Bangla (`bn-BD`). All visible text goes through Fluent keys in
  `assets/i18n/en-US/main.ftl` and `assets/i18n/bn-BD/main.ftl`, used as `{{ t(key="login-title", lang=tag) }}`.
  Add every key to both files. Keys are `area-meaning` in kebab-case (`contact-error-email`).
- Admin screens are English only.
- **Content translations** (database content such as pages): English is required; every other language is
  optional. An empty part is stored as NULL and visitors see the English text for that part (title, description
  and text each fall back on their own). Clearing a whole language deletes its row.
- Styling is Tailwind with the project colours (`brand-navy`, `brand-amber`). Rebuild the CSS after adding classes.
- Use `| safe` only for HTML that was cleaned with `ammonia` on the way in.

## Forms, components and modals

- State-changing requests are POST forms that call `same_origin(&headers)` first and answer 403 if it fails.
- Redirect after a successful POST (303 See Other).
- **Components (the Livewire/Volt idea):** a part of a page that updates without a reload is a Tera partial.
  The form has `data-component`, the partial's root has `data-component-root`, and `assets/static/js/components.js`
  posts it with `fetch` and the header `X-Component: 1`. The server checks that header: with it, return just the
  partial; without it, return the full page, so the form still works with JavaScript off. See
  `modules/contact` (`public.rs`, `templates/contact/_form.html`).
- **Modals:** an Alpine `x-data` on the list page fetches the fragment (`GET /admin/things/{id}` with
  `X-Component: 1`) into a dialog; actions inside the dialog are `data-component` forms. After a swap the script
  fires a `component:updated` event so the list can refresh its badge.
- **Spam:** public forms get a honeypot (a field called `website`, hidden off-screen). When it is filled, answer
  exactly like success and store nothing. Add a per-IP rate limit through the cache and the origin check.

## JSON API

- Everything under `/api/v1`. Versioned, JSON only, never an HTML page (unknown `/api/v1/auth/*` paths answer a
  JSON 404).
- Authentication is a bearer token in the form `{id}|{secret}` (`Authorization: Bearer ...`). Protect a route by
  taking `ApiAuth(user)` as an argument; do not read the header yourself.
- Responses follow Laravel's shapes so a Laravel-minded client works unchanged:
  - success: `{ "data": ... }` for one thing or a list, or `{ "user": ..., "token": ... }` for sign-in
  - error: `{ "message": "..." }` with 401 `Unauthenticated.`, 403, 404 `Not found.`, 429
  - validation: 422 `{ "message": "...", "errors": { "field": ["..."] } }`, the message being the first error
    plus "(and N more errors)"
- Read bodies leniently (a bad body behaves like `{}` and gives a validation error, not a parser error).
- Sign-in endpoints are throttled (5 failures lock the account for 15 minutes) and treat "unknown email" and
  "wrong password" identically (same message, same timing).
- Return only what the client needs: never password hashes, token hashes or `api_key`.

## Database

- MariaDB/MySQL. Tables are plural `snake_case`; columns `snake_case`; every table has `id`, `created_at`,
  `updated_at` unless it is a link table.
- A migration lives in the module that owns the table, as
  `m{YYYYMMDD}_{six digit sequence}_{what_it_does}.rs`, and is registered in that module's `migrations/mod.rs`.
  Migrations of all modules are merged by name.
- **One migration file per table, in this starter.** The base is clean: every table is created by exactly one
  file and never altered by a later one (no `ALTER TABLE`, no "add column" files). While the starter is still
  being shaped, change the table's own migration and recreate the local database (`cargo run -- db reset`, then
  `db seed`). `tests/migrations.rs` enforces this and also checks that sequence numbers are unique.
  Someone who clones the starter and ships it should add **new** migration files for their own changes and never
  edit one that has already run on a shared or production database.
- Give every migration a unique, increasing sequence number (`m20261030_000035_...`); pick the next free one.
- Migrations that must work on a database also used by Laravel use `CREATE TABLE IF NOT EXISTS` and plain SQL.
- **Laravel compatibility** (the apps may share a database):
  - `media` is Spatie laravel-medialibrary's table, unchanged.
  - Passwords are Argon2id (the Rust app does not verify bcrypt). Hash with `loco_rs::hash::hash_password`.
  - `users.pid` is 16 raw bytes (a binary UUID).
  - Unsigned integer columns map to unsigned Rust types.
- Keep ordering and uniqueness in the database (unique indexes), not only in code; handle the "two requests at
  once" case by catching the unique violation.

## Media library

- Files belong to a model through `HasMedia`/`Owner` and live in named **collections**, like Spatie. A
  single-file collection (`CollectionDef::single`) replaces the old file when a new one is added; a multiple one
  (`CollectionDef::multiple`) can keep the newest N.
- Declare accepted types and size per collection, and mark a collection `.public()` only if anyone may see it.
  Everything else is private (owner and staff with `media.view`).
- `model_type` is a short stable name (`user`, `page`, `library`) or whatever the Laravel app stored. Never store
  a path or URL in your own table; store the media relation and ask for the URL.
- Files sit at `{id}/{file_name}`. Where they are kept is chosen in `.env`: `MEDIA_STORAGE=local` (`storage/uploads`) or `r2` (the `R2_PUBLIC_BUCKET`), with `MEDIA_DISK`, `MEDIA_PREFIX` and `R2_PUBLIC_URL`. Never put private collections in a bucket that has public access.
- Upload limits: 10 MB per file (`DEFAULT_MAX_BYTES`) and the request body limit in `config/*.yaml`
  (`limit_payload`, 11 MB). Raise both together. A reverse proxy has its own limit too.
- Details: `tutorials/17-media-library.md`.

## Security

- Passwords: Argon2id, minimum 8 characters, never logged or returned.
- Sessions and tokens: store only a hash (`SHA-256`) of the secret; compare hashes, never plain values.
- CSRF: `same_origin` on every state-changing form request plus `SameSite=Lax` cookies. Cookies are `HttpOnly`
  and `Secure` in production.
- Authorization before data: check the permission first, then load. Unknown and forbidden private resources both
  answer 404 when their existence is sensitive (private media).
- User text is escaped by Tera. Staff HTML goes through `ammonia` before it is stored.
- Never invent security: use the existing extractors (`Auth`, `AdminPage`, `ApiAuth`) and helpers.
- No secrets in git: `.env`, keys and tokens stay out. Scan the diff before you push.
- Behind a proxy, rate limits read `X-Forwarded-For`; that header is only trustworthy if your proxy sets it.

## Audit trail, background jobs and backups

- **Log every admin change.** A handler that changes data writes an activity entry after the change, with
  `loco_activity_log::activity(&ctx.db)...log_quietly("...")`: a log name for the area (`users`, `roles`, `pages`),
  `caused_by("user", id)`, `performed_on(type, id)`, an `event`, and `changes(&old, &new, &[])` for what changed.
  Never put passwords, tokens or other secrets in an entry. Use `log_quietly` in handlers so a log problem cannot
  break the action.
- **Packages hold shared writers.** `packages/loco-activity-log` is the writer, so a module low in the dependency
  graph can log without depending on the admin module.
- **Wrap worker bodies in `module_queue_watcher::track(...)`** so every run shows up in the queue monitor with its
  status, time and error.
- **Backups** are configured in `settings.db_backup`, separately from the website's `storage:`; keys come from the
  environment with `get_env`, never from the file. Restore steps live in the module README.

## Email and PDF

- **Send mail only through `module_mail::send`** so every attempt is logged in `mail_log`. Build mails as classes with
  `cargo modular make:mail Name --module=<module>` (HTML and text templates; never string-build HTML in a handler).
- Mail text and subjects for people use Fluent keys like the rest of the site; never put secrets or tokens that are not
  needed in an email.
- `MAIL_DRIVER` is `log` (writes `.eml` files) everywhere except where real mail is meant; tests use `capture`. Do not
  send real mail from tests.
- **PDFs** are HTML and CSS rendered by `module_pdf::html_to_pdf` (Chrome). Page size and margins are CSS `@page`. Keep
  money in minor units (integers), never floats.
- A feature that emails a PDF renders the PDF first, attaches it, and writes an activity-log entry for the send.

## AI providers

- **Call AI only through `module_ai_service::AiService`** (`chat`, `json`, `generate_image`). Never call a provider from
  another module: the wrapper picks the provider and model from `.env`, logs the call and estimates the cost.
- Keys and model names live in `.env` only (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`, `*_MODEL`).
- Give every call a `purpose("...")` so the AI API logs say what it was for.
- Anything about a person (passports, IDs, faces, health) is `.sensitive()`: the prompt and reply are not stored.
  Pictures are never stored.
- Ask for JSON with `ai.json(...)` and check the fields you need; a model's reply is input, not truth.

## Console commands and the scheduler

- A command is a Loco task named `<module>:<action>` (`pages:check-translations`, `auth:prune-sessions`).
  Create it with `cargo modular make:command <Name> --module=<module>`.
- Arguments are `key:value`. A command that writes should print what it did and return an error (non-zero exit)
  on failure.
- A module declares schedules in `fn schedule(&self)`, for example `ScheduledJob::command("x:y").daily_at(3, 0)`.
- Consoles for people: `task tinker` (SQL) and `task tinker:eloquent` (Eloquent and the `DB` facade; PHP needed).

## Tests

- Feature tests go in `tests/requests/<feature>.rs` and are registered in `tests/requests/mod.rs`. Task tests
  go in `tests/tasks/`. Unit tests of pure functions sit next to the code in `#[cfg(test)] mod tests`.
- Mark every test that touches the database `#[serial]` and build the app with `request::<App, _, _>(...)` or
  `boot_test::<App>()`.
- Test behaviour through HTTP: status codes, redirects, and the HTML or JSON the user would see. Assert on
  `data-testid` values and visible text, not on class names.
- Cover for each feature: the happy path, validation errors, permission (a person without the right gets 403),
  a forged origin, and the "nothing was changed" case after a refusal.
- Test files re-declare small helpers (`login`, `user`, `post`) rather than sharing a module; copy the pattern
  from `tests/requests/admin_users.rs`.
- When a change alters counts or text that other tests assert on (sidebar links, permission counts), update
  those tests in the same change.
- One test run at a time (shared database).

## Documentation and git

- Update, in the same change: the module's `README.md`, the matching tutorial in `tutorials/`, and the root
  `README.md` module table when you add a module. Add new conventions here.
- Tutorials are numbered files with a navigation bar; keep the bar when you edit them.
- Commit messages: a short imperative subject, then a bullet list of what changed and why. Do not add
  attribution lines for tools or assistants.
- One logical change per commit, only after fmt, clippy and tests pass. Check `git status` for strays
  (`target/`, logs, `php/vendor`) and scan the diff for secrets before pushing.
- Deployment: build and test locally, push with git, and update the server with `git pull --ff-only`. Never
  force-push, hard-reset or overwrite the server's checkout.

## Checklists

**A new admin resource** (list, show, edit):

- [ ] entity and migration in the owning module
- [ ] permissions `<area>.view` / `.manage` in `definitions.rs`, granted to roles, `roles:sync` run
- [ ] routes `GET /admin/x`, `/admin/x/{id}`, `/admin/x/{id}/edit`, `POST /admin/x/{id}` (+ actions)
- [ ] templates registered in `templates()`, breadcrumb block, ID first column linking to show
- [ ] `admin_menu()` entry
- [ ] flash messages through `?saved=` / `?error=`
- [ ] tests: list, show, edit, validation, permission, forged origin
- [ ] README and tutorial updated

**A new public form:**

- [ ] partial plus full page, `data-component` form, `X-Component` handled
- [ ] honeypot, origin check, rate limit, server validation with typed values kept
- [ ] all text through Fluent keys in both languages
- [ ] tests for success, errors, honeypot, throttle, forged origin

**A new API endpoint:**

- [ ] under `/api/v1`, `ApiAuth` if it needs a person
- [ ] Laravel-shaped success and error JSON, lenient body parsing
- [ ] throttle anything that checks credentials
- [ ] tests for 401, 403, 422, success

**A new module:**

- [ ] `cargo modular make:module`, README, entry in the root README module table
- [ ] no dependency cycle (see [Modules](#modules))
- [ ] migrations, entities, routes, templates, permissions, tests, tutorial notes
