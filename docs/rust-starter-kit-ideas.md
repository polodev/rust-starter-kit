# Rust Starter Kit: Feature Ideas

Stack: Axum, Loco, SeaORM on **MariaDB only**, Tera (Loco's built-in view engine), **Tailwind CSS**, HTMX + Alpine.js.
Goal: build admin and public apps with Laravel-like speed.

The kit is a **Cargo workspace of modules**, the same way Bimafy and Dokani use `app-modules/` (internachi/modular). Four modules are ported from your existing projects. Everything else is built on top of them.

| Rust module | Ported from | What it carries |
|---|---|---|
| `role-permission` | `dokani/app-modules/role-permission` | Config-defined roles and permissions, sync, middleware, route audit, admin panel |
| `api` | `dokani/app-modules/mobile-api` (plus Bimafy `api-access` and `api-log`) | Versioned JSON API, device tokens, response envelope, sync, API client access, request logging |
| `admin-layout` | `bimafy/app-modules/ui-backend` | Admin layout: topnav, sidebar, content area, toast, flash |
| `site-layout` | `bimafy/app-modules/ui-frontend` | Public layout and reusable form/modal components |

---

## 1. Module system (the Laravel `app-modules` equivalent)

Loco has no module concept, so the kit adds a thin convention.

```
starter-kit/
  Cargo.toml                (workspace)
  src/                      (thin Loco app: wires modules together)
  modules/
    role-permission/
    api/
    admin-layout/
    site-layout/
    <your-feature>/         (created by the module generator)
  xtask/                    (generators: make:module, make:resource, ...)
```

Each module crate exposes the same small surface:

```rust
pub fn routes() -> loco_rs::controller::AppRoutes;
pub fn migrations() -> Vec<Box<dyn sea_orm_migration::MigrationTrait>>;
pub fn register_tasks(tasks: &mut loco_rs::task::Tasks);
pub async fn register_workers(queue: &loco_rs::bgworker::Queue, ctx: &AppContext) -> Result<()>;
pub fn permissions() -> Vec<PermissionDef>;      // merged into role-permission
pub fn sidebar() -> Vec<SidebarItem>;            // merged into admin-layout
pub fn config_defaults() -> serde_json::Value;
```

The app crate's `Hooks` impl and `Migrator` just loop over the module list, so adding a module means one line in one place. Module template folders follow the Laravel layout: `src/`, `templates/`, `migrations/`, `config/`, `tests/`.

---

## 2. `role-permission` module (from Dokani)

Dokani's design is the one to port. Key behaviours:

- **Config is the single source of truth.** `config/permissions.php` and `config/roles.php` become `permissions.rs` or `permissions.yaml` and `roles.rs`. Edits take effect after `role-permission:sync`.
- **Permissions are generated per area:** `area.view`, `area.manage`, plus one-off keys like `roles.assign`, `invoices.discount`, `sales.delete`.
- **Ranked roles with inheritance.** Each role includes everything the roles below it have (`new_employee` < `business_development` < `employee` < `admin` < `developer`). Sync validates the ordering.
- **Wildcards:** `crm.*` grants every `crm.` permission.
- **One role per user, plus direct permission exceptions.** `users.id` unique in `role_user`, extra grants in `permission_user`.
- **Protected permissions:** `roles.*` cannot be granted as direct exceptions. Only the top role can assign roles or run sync from the panel.
- **Sync deactivates removed definitions** (`is_active = false`) instead of deleting rows, so history survives.
- **Fail-closed route authorization.** A resolver maps each admin route to a permission by area. A command (`advanced-authorization:audit --fail-on-unmapped`) fails CI when an admin route has no mapping. Unmapped admin routes are denied at runtime.
- **Per-request permission cache** on the user, with `forget_permission_cache()` when roles change.
- **Console commands:** `role-permission:sync`, `advanced-authorization:audit`, `advanced-authorization:verify`.
- **Admin panel (six parts):** Overview, Roles, Permissions, Permission Matrix, User Assignments, Backend Links.

Tables: `roles` (slug, name, rank, description, is_active), `permissions` (key, name, group, description, is_active), `permission_role`, `role_user`, `permission_user`. Table names are configurable.

Rust shape:

- SeaORM entities for the five tables plus `user_ext` trait: `user.has_permission("crm.view")`, `user.has_role("admin")`.
- Axum layer `require_permission("crm.view")` and an extractor `Authorized<"crm.view">`.
- Route-level auto-check layer for `/admin/*` using the resolver (fail closed).
- Tera function `{% if can(permission="crm.view") %}`, the equivalent of `@backendPermission`.
- Loco task `role_permission_sync` and `authorization_audit`.
- Optional per-model policies (`can_update(user, model)`) on top.

---

## 3. `api` module (from Dokani mobile-api, with Bimafy api-access and api-log)

Everything lives under `/api/v1`. It is a real Laravel-style API module with its own routes, middleware, controllers, resources and docs.

**Response envelope (from `ApiResponse`):**

```json
{ "success": true, "data": {}, "meta": { "server_time": "2026-10-30T10:00:00Z" } }
{ "success": false, "error": { "code": "TOKEN_EXPIRED", "message": "...", "details": {} }, "meta": { "server_time": "..." } }
```

Stable error codes: `UNAUTHENTICATED`, `TOKEN_EXPIRED`, `ACCESS_REVOKED`, `BUSINESS_INACTIVE`, `VALIDATION_FAILED`, and so on. Clients switch on `code`, never on message text.

**Auth and devices (from `AuthenticateMobileDevice`, `MobileAccessToken`, `MobileDevice`):**
- Login returns a random plain token once. Only `sha256(token)` is stored.
- `access_tokens`: `device_id`, polymorphic `actor` (user or another account type), `token_hash`, `last_used_at`, `expires_at`, `revoked_at`.
- `devices`: platform, app version name and code (from `X-App-Version-*` headers), `last_seen_at`, `revoked_at`.
- Middleware checks token, expiry, revocation, actor still valid, tenant still active; it then touches `last_used_at` and `last_seen_at`.
- TTL from config (`API_TOKEN_TTL_DAYS`, default 90).
- Social login hook (Google ID-token exchange) as an optional feature.

**Rate limits per route group** (from the Dokani routes): login 10 per minute, public config 60 per minute, authenticated group 120 per minute.

**Sync endpoints (offline-first clients):**
- `POST /sync/push`: up to N operations per call (`max_push_operations`), each with a client UUID so retries are idempotent. Stored in a `mutations` table.
- `GET /sync/pull?cursor=`: paged changes feed (`pull_page_size`), backed by a `changes` table filled by model observers (`ChangeRecorder`).
- Dataset policy decides which tables a given actor may sync.

**Other ready-made endpoints:**
- Releases: `GET /releases/check`, signed-URL download, admin upload of release artifacts.
- Media upload with request-status polling (`/requests/{uuid}`) for slow jobs.
- Locale middleware (`Accept-Language` or `X-Locale`).

**Client access control (from Bimafy `api-access`):** `api_accesses` table (company, owner, product model, `is_active`) plus middleware for third-party partner APIs that use a key instead of a device token. Admin UI to create, disable and view partner access.

**Request log (from Bimafy `api-log`):** `incoming_api_access_logs` with IP, user agent, endpoint, method, key, `access_status` (`success`, `missing_api_key`, `invalid_api_key`, `expired_api_key`, `rate_limit_exceeded`, `blocked_whitelist`), error code and message, request headers and body. Header redaction for secrets. Admin viewer with filters. Retention task prunes old rows.

**API docs:** generated OpenAPI (`utoipa`) at `/api/v1/docs`, plus a hand-written `docs/API.md` like Dokani's `MOBILE_API.md`.

**Resources:** one `XResource` struct per entity (the Laravel API Resource equivalent) so JSON shape is not tied to the DB model.

**Tests:** each endpoint needs a feature test for auth failure, revoked token, validation error, and rate limit.

---

## 3a. Tenancy: single app, tenants resolved by URL (the Dokani way)

One codebase, one MariaDB database, shared tables. There is no per-tenant database or subdomain. A tenant (Dokani calls it a business or organization) is identified by the URL, and every tenant-owned row carries a `tenant_id`.

Dokani's URL patterns to copy:

| Purpose | Pattern | Dokani example |
|---|---|---|
| Tenant management (authenticated) | `/business/{tenant_id}/...` and `/business/{slug}/...` | `business/{businessSlug}/dashboard`, `business/{businessId}/settings` |
| Nested resource inside a tenant | `/business/{slug}/shops/{shopSlug}/...` | shops, employees |
| Sub-tenant app surface | `/{tenantSlug}/{childSlug}` | POS at `/{businessSlug}/{shopSlug}` |
| Public pages | `/{id}` or `/{slug}` | storefront; slug redirects to the id URL, which is canonical |

Kit pieces:
- `tenants` table (id, slug unique, name, status, settings JSON) and `tenant_user` access table (user, tenant, access level). The user can belong to several tenants.
- `resolve_tenant` middleware/extractor: numeric segment resolves by id, otherwise by slug. It stores the tenant in request extensions, or returns 404.
- `canonical_tenant` middleware: public slug URLs redirect (301) to the id URL, matching Dokani.
- `ensure_tenant_access` middleware: the signed-in user must have a `tenant_user` row for this tenant. Suspended tenants get a `TENANT_INACTIVE` error (same idea as Dokani's `BUSINESS_INACTIVE`).
- `TenantScoped` helper for SeaORM: every query on a tenant-owned entity goes through `Entity::for_tenant(&ctx)` so `tenant_id` is always filtered. Entities created with `make:resource --tenant` get the column, index and scoping for free.
- Route helper `tenant_route("name", &tenant)` so templates never build URLs by hand.
- Reserved-slug list (`admin`, `api`, `login`, ...) so a tenant slug cannot shadow an app route.
- Role-permission stays **platform-level** (staff roles, as in Dokani). Per-tenant access (owner, employee) is separate and lives in `tenant_user`.
- API tokens carry the tenant on the device record, like Dokani's `device.business`.
- Cache keys, storage paths (`tenants/{id}/...`) and settings are prefixed by tenant.

---

## 4. `admin-layout` module (from Bimafy)

Bimafy's admin layout is AdminLTE 4 on Bootstrap 5. The kit keeps the same structure (topnav, collapsible sidebar, content area, toast) but builds it with **Tailwind CSS** classes, using the Tailwind standalone CLI so no Node is needed.

What Bimafy's layout contains, and how to port it:

| Bimafy piece | Rust equivalent |
|---|---|
| `components/layout` + `layouts/app.blade.php` (head, topnav, sidebar, `@yield('content')` or slot, footer, scripts) | Tera `base_admin.html` with blocks `title`, `head`, `content`, `scripts` |
| `_topnav` with permission-gated links | `topnav` built from a registry, filtered by `can()` |
| `_sidebar` with user panel (avatar, name, role names, logout) | `sidebar.html` fed by `CurrentUser` extractor |
| `navsidebar/_blog.blade.php`, `_queue_watcher...` (one partial per feature, each wrapped in `@backendPermission`) | Each module returns `Vec<SidebarItem>` from `sidebar()`; the layout renders the merged, permission-filtered tree |
| `Helpers::setActiveByRouteName` | `active(routes=[...])` Tera function marking `menu-open` / `active` |
| Toast container | `toast.html` + flash messages from session |
| `Helpers::dashboardLink()` | Role-aware landing route helper |

Additions for the kit:
- `SidebarItem { label, icon, route, permission, children, order }` registry, so a new module appears in the menu with no template edits.
- Reusable partials: data table, pagination, filter bar, confirm-delete modal, form field components, breadcrumb.
- HTMX + Alpine for inline search, modals and partial reloads (the Livewire/Volt feel without a JS build).
- Tailwind standalone CLI builds one CSS file (`tailwind.config.js` scans the Tera templates of all modules). Shared component classes live in one `components.css`.
- Dark mode toggle, remembered per user.

---

## HTMX: the Livewire-like layer

Loco has no Livewire equivalent. Rust options are:

- **HTMX + Alpine.js (chosen).** The server returns HTML fragments from Tera partials and HTMX swaps them in. This is the closest match to Livewire's "state lives on the server" model, with no JS build step.
- Leptos / Dioxus / Yew: Rust compiled to WASM. A different model (a SPA), more work, and not compatible with Tera views.
- Datastar: similar idea to HTMX, using SSE. Smaller community.

How the kit uses HTMX:
- Detect `HX-Request` in an extractor so one handler returns the full page normally and only the partial for HTMX calls.
- Livewire patterns mapped over: `wire:model.live` -> `hx-trigger="keyup changed delay:300ms"`, `wire:click` -> `hx-post`, `wire:loading` -> `htmx-indicator`, `wire:poll` -> `hx-trigger="every 5s"`, events -> `HX-Trigger` response header.
- Components: live search table, inline edit row, modal loaded from a URL, confirm delete, infinite scroll, toasts via `HX-Trigger`.
- Validation errors return the re-rendered form partial with status 422.
- Alpine.js handles purely client-side state (dropdowns, tabs).

---

## 5. `site-layout` module (from Bimafy)

- `base_public.html` layout: header, footer, SEO block (title, description, canonical, Open Graph), flash messages.
- Component set: input, label, input-error, button, secondary/danger button, modal, dialog-modal, confirmation-modal, dropdown, nav-link, banner, action-message, validation-errors, authentication card, form-section.
- Auth pages (login, register, forgot password, profile) using those components.
- Public-site helpers: sitemap, robots, localized routes.

---

## 6. CRUD speed (the "Laravel quick" part)

- `cargo xtask make:module <name>` scaffolds a module crate with the surface from section 1.
- `cargo xtask make:resource <module> <name> --fields title:string,price:decimal` generates model, migration, controller, list/create/edit templates, request validation, permissions (`name.view`, `name.manage`), sidebar entry, API resource and a test file.
- Validation extractor `ValidatedForm<T>` that re-renders the form with errors and old input.
- List helper for search, sort, filters and pagination from query params.
- Soft deletes, ULID/UUID, timestamps and activity logging as SeaORM derive/helpers.
- Model events (`created`, `updated`, `deleted`) with an observer registry, used by the API change feed and activity log.

---

## 7. Loco features to expose and wrap

| Laravel | Loco / kit wrapper |
|---|---|
| Queue and jobs | Loco workers; kit adds failed-jobs table, retry, and admin dashboard |
| Scheduler | `scheduler.yaml`; kit adds admin page with last run, status, next run |
| Mail | Loco mailers; kit adds notification fan-out (mail, database, Telegram, SMS) |
| Storage | Loco storage; kit adds named disks (`public`, `private`, `s3`, `r2`), presigned URLs, upload validation |
| Media library | Polymorphic `media` table (`model_type`, `model_id`, `collection`), thumbnails as queued job |
| Cache | Loco cache; kit adds tagged keys for permission cache invalidation |
| Tinker | Loco task / small REPL task for quick queries |
| Seeders and factories | Loco seeds + `fake` crate factories |

---

## 8. Production concerns

- Sessions for admin (cookie, secure, CSRF) and tokens for API, side by side.
- Settings table with typed `setting("site.name")` helper and admin page.
- Activity/audit log (who changed what, from observers).
- Rate limiting, CORS, security headers.
- i18n (Bimafy and Dokani both use localization).
- Health check, graceful shutdown, tracing, optional Sentry/OTel.
- Docker Compose with MariaDB, Redis, MinIO and Mailpit.
- `just` commands: `dev`, `migrate`, `seed`, `fresh`, `test`, `audit`.
- Test harness: a dedicated MariaDB test database (never the dev or production one). A boot guard aborts tests unless the database name ends in `_test`, the same idea as Bimafy's database safety guard. Includes an `acting_as(user)` helper and per-test transaction rollback or truncate-on-setup.
- MariaDB notes: SeaORM uses its MySQL driver (`mysql://` URL, utf8mb4). Use `ULID`/`UUID` as `CHAR(26)`/`CHAR(36)` or `BINARY(16)`; JSON columns map to MariaDB `LONGTEXT` with JSON check; avoid Postgres-only types and `RETURNING` assumptions. Queue backend is Redis or in-process, not the DB, unless you want a DB-backed queue table.

---

## 9. Build order

1. Workspace, Loco app, module convention, Tera setup (custom functions: `can`, `active`, `route`), `make:module` generator.
2. `admin-layout` and `site-layout` layouts with the sidebar registry.
3. Auth pages, sessions, user model.
4. `role-permission` (tables, sync, extractors, audit command, admin panel).
5. `make:resource` generator. This is the main speed gain.
6. `api` module (envelope, device tokens, throttling, resources, docs).
7. API sync endpoints, partner access (`api-access`) and request log (`api-log`).
8. Media/storage, settings, activity log, notifications.
9. Failed jobs and scheduler admin pages.

---

## 10. Decisions made

- Database: **MariaDB only**.
- Styling: **Tailwind CSS**.
- Tenancy: single codebase and database, tenants resolved by URL as in Dokani (section 3a).
- Views: Tera. Interactivity: HTMX + Alpine.js.

## 11. Still open

- Should the kit ship the offline sync endpoints, or only the plain REST API?
- Canonical tenant URL: id-based (Dokani's rule) for both management and public pages, or id only for public pages?

## 12. Session types (decided)

- **Short-lived:** query-result caching, the `Cache::remember` equivalent. Built as `loco_modular::remember`.
- **Long-lived:** login and language selection. Language is a one-year `lang` cookie (built). Login session (signed cookie plus DB-backed session record, refresh on use) is still to build.

## 13. Status

Done: `cargo modular` CLI (`make:module`, `make:command`, `modules:list`, `modules:sync`, `command:list`, `command:run`, `schedule:list`, `schedule:work`), module-declared schedules, `loco-modular` runtime, Loco app on MariaDB, `site-layout`, `pages` (en + bn seeder), Tailwind build, test guard.
Not done: `admin-layout`, `role-permission`, `api`, login sessions and auth pages, tenancy, `make:resource` and the other generators, media/storage code, settings, activity log, queue and scheduler pages.
