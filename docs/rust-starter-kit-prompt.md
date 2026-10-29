# Prompt: Build the Rust Starter Kit

Copy everything below the line into a new Claude Code session, started in an empty directory. For the full feature rationale see `rust-starter-kit-ideas.md`.

---

You are building a **Rust starter kit** that gives Laravel-like development speed. Work in this empty directory.

## Stack (fixed)
- Rust (stable), **Loco** (loco-rs), **Axum** (via Loco), **SeaORM** + sea-orm-migration, **Tera** templates (Loco's built-in view engine; do not add Tera).
- HTMX + Alpine.js for interactivity (the Livewire-like layer): handlers detect `HX-Request` and return Tera partials; use `HX-Trigger` response headers for toasts/events and status 422 with a re-rendered form partial for validation errors. No Node in production.
- **MariaDB only** (SeaORM MySQL driver, `mysql://`, utf8mb4) for dev, test and production. Tests use a separate MariaDB database whose name must end in `_test`.
- **Tailwind CSS** via the standalone CLI (no Node).

## Reference projects (read before writing code)
Read these directly; port the design, not the PHP syntax.

1. **Role permission:** `~/sites/dokani/app-modules/role-permission`
   - `README.md`, `config/permissions.php`, `config/roles.php`, the migration `*_create_role_permission_tables.php`
   - `src/Concerns/HasAdvancedPermissions.php`, `src/Services/RolePermissionSyncService.php`, `src/Services/AdvancedRoutePermissionResolver.php`
   - `src/Http/Middleware/*`, `src/Console/Commands/*`, `src/Http/Controllers/RolePermissionController.php`, `resources/views/*`
2. **API:** `~/sites/dokani/app-modules/mobile-api` and `~/sites/dokani/docs/MOBILE_API.md`
   - `routes/mobile-api-routes.php`, `config/mobile-api.php`, `src/Support/ApiResponse.php`
   - `src/Http/Middleware/AuthenticateMobileDevice.php`, `src/Models/MobileAccessToken.php`, `MobileDevice.php`
   - `src/Services/MutationProcessor.php`, `ChangeRecorder.php`, `SnapshotService.php`, `AppReleaseCatalog.php`
   - Also `~/sites/bimafy/app-modules/api-access` (partner access) and `~/sites/bimafy/app-modules/api-log` (request log)
3. **Admin and public UI:** `~/sites/bimafy/app-modules/ui-backend` and `~/sites/bimafy/app-modules/ui-frontend`
   - `ui-backend/resources/views/layouts/app.blade.php`, `layouts/partials/_head, _topnav, _sidebar, _footer, _script`, `layouts/partials/navsidebar/*`
   - `ui-frontend/resources/views/components/*`, `layouts/app.blade.php`
4. **Module convention:** how `~/sites/bimafy/app-modules/*` is structured (`src/Providers`, `routes`, `resources/views`, `database/migrations`, `config`, `tests`).

## Architecture
Create a **Cargo workspace**:

```
Cargo.toml                workspace
src/                      thin Loco app that wires modules
modules/
  role-permission/
  api/
  admin-layout/
  site-layout/
xtask/                    generators
```

Every module crate exposes the same surface (implement a shared trait or plain functions, your call, but keep it uniform):
`routes()`, `migrations()`, `register_tasks()`, `register_workers()`, `permissions()`, `sidebar()`, `config_defaults()`.
The app crate's Loco `Hooks` and `Migrator` iterate over a single module list, so adding a module is a one-line change.

## Deliverables

### A. `role-permission` module
Port Dokani's design:
- Tables: `roles`, `permissions`, `permission_role`, `role_user` (unique `user_id`: one role per user), `permission_user` (direct exceptions). Table names configurable.
- Permissions and roles defined in code/config; roles are ranked and inherit from lower roles; wildcard `area.*` supported.
- A sync routine (also a Loco task `role_permission_sync`) that creates/updates definitions, validates rank order, and **deactivates** removed definitions instead of deleting.
- Protected permissions (`roles.*`) cannot be granted as direct exceptions; only the top role assigns roles.
- User extension trait: `has_permission`, `has_role`, `roles`, `forget_permission_cache`, with an in-request cache.
- Axum pieces: `require_permission("x")` layer, an `Authorized` extractor, and a Tera function `can(permission="x")`.
- Fail-closed route authorization for `/admin/*` through a resolver that maps route area to permission, and a Loco task `authorization_audit` that exits non-zero when an admin route is unmapped.
- Admin panel pages: Overview, Roles, Permissions, Permission Matrix, User Assignments, Backend Links (all built with the `admin-layout` layout).
- Tests: inheritance, wildcard, direct exception, protected permission, unmapped route denied, sync deactivation.

### B. `api` module
Under `/api/v1`:
- Response envelope exactly like Dokani's `ApiResponse` (`success`, `data`, `error{code,message,details}`, `meta.server_time`) and a stable error-code enum.
- Device-token auth: login issues a random token returned once; store only `sha256(token)`. Tables `devices` and `access_tokens` (polymorphic actor, `expires_at`, `revoked_at`, `last_used_at`). Middleware validates token, expiry, revocation, actor, and updates last-seen plus `X-App-Version-*` headers.
- Per-group rate limits (login 10/min, public 60/min, authenticated 120/min), configurable.
- Locale middleware.
- Sync: `POST /sync/push` (batched, idempotent by client UUID, `mutations` table) and `GET /sync/pull?cursor=` (paged `changes` feed filled by model observers).
- Release check and signed download endpoints.
- Partner access: `api_accesses` table, middleware, admin CRUD (port `api-access`).
- Request log: `incoming_api_access_logs` table, middleware that records every attempt with a status (`success`, `missing_api_key`, `invalid_api_key`, `expired_api_key`, `rate_limit_exceeded`, `other_error`), secret redaction, retention task, admin viewer (port `api-log`).
- One Resource struct per entity for JSON output, OpenAPI via `utoipa` at `/api/v1/docs`, and `docs/API.md`.
- Tests per endpoint: unauthenticated, expired token, revoked token, validation error, rate limit, success.

### C. `admin-layout` module
Port Bimafy's admin layout:
- Tera `base_admin.html` (head, topnav, sidebar, content block, footer, scripts, toast container, flash messages).
- Sidebar built from a **registry**: each module's `sidebar()` returns `SidebarItem { label, icon, route, permission, children, order }`; the layout merges them, filters by `can()`, and marks active/open items (equivalent of `setActiveByRouteName`).
- Sidebar user panel (avatar, name, role names, logout).
- Look: same structure as Bimafy's AdminLTE layout, built with Tailwind CSS classes. Dark mode toggle.
- Reusable partials: data table, pagination, filter bar, confirm-delete modal, form-field components, breadcrumbs.
- HTMX for inline search/pagination and modal loading.

### D. `site-layout` module
- `base_public.html` with SEO block (title, description, canonical, OG), header, footer, flash.
- Component set ported from Bimafy's `ui-frontend/components` (input, label, input-error, buttons, modal, dialog/confirmation modal, dropdown, nav-link, banner, action-message, validation-errors, auth card, form-section).
- Auth pages: login, register, forgot/reset password, profile (session cookie, CSRF).

### E. Generators (`xtask`)
- `cargo xtask make:module <name>`: scaffold a module crate with the uniform surface, templates folder, migrations folder, and a test file.
- `cargo xtask make:resource <module> <Name> --fields title:string,price:decimal,...`: generate entity, migration, controller, list/create/edit Tera templates, `ValidatedForm` request struct, permissions (`name.view`, `name.manage`), sidebar item, API Resource struct, and a feature test.

### F. Shared support
- `ValidatedForm<T>` extractor: re-renders the form with errors and old input.
- List helper: search, sort, filter, pagination from query params.
- SeaORM helpers: soft delete, ULID/UUID, timestamps.
- Model observer registry (used by the API change feed and the activity log).
- Settings table with typed `setting("key")` helper and admin page.
- Activity log (who changed what).
- Storage: named disks (`public`, `private`, `s3`, `r2`) on top of Loco storage, upload validation extractor, presigned URLs.
- Queue/scheduler: example worker, failed-jobs table with retry, admin page showing jobs and scheduled tasks.
- Seeders: one developer user, default roles.

## Working rules
- **Plan first.** Before coding, give me a short plan: crate layout, the exact module trait, and the migration order. Wait for my OK.
- Build in this order and stop after each step for my review:
  1. Workspace, Loco app, module convention, Tera setup (custom functions `can`, `active`, `route`), `make:module`.
  2. `admin-layout` + `site-layout` layouts and sidebar registry.
  3. Auth pages, sessions, user model.
  4. `role-permission`.
  5. `make:resource` generator.
  6. `api` (envelope, tokens, throttling, resources, docs).
  7. API sync, partner access, request log.
  8. Media/storage, settings, activity log, notifications, failed jobs and scheduler pages.
- Follow Loco conventions (`cargo loco generate ...` where it fits) and check sibling code before inventing a new pattern.
- Write tests with every module. Tests must use the dedicated MariaDB test database and a boot guard must abort unless the database name ends in `_test`.
- No destructive database commands (no drop, reset or fresh) against anything except the test database.
- Run `cargo fmt`, `cargo clippy -D warnings`, and `cargo test` before reporting a step done. Report failures with their output.
- Keep explanations short. Do not create extra documentation files beyond `README.md`, `docs/API.md`, and per-module `README.md`.
- Do not add dependencies beyond what the stack needs without telling me why.

## Tenancy (single tenant app, multi-tenant by URL, the Dokani way)
One codebase, one MariaDB database, shared tables; every tenant-owned row has `tenant_id`. Read `~/sites/dokani/TARGET_ARCHITECTURE.md` ("Organization URL Strategy") and `~/sites/dokani/app-modules/business/routes/business-routes.php` and `~/sites/dokani/app-modules/shop/routes/shop-routes.php` first.
- Tables: `tenants` (id, slug unique, name, status, settings JSON), `tenant_user` (user, tenant, access level).
- URL patterns: `/business/{tenant}/...` for management, nested `/business/{tenant}/shops/{child}/...`, `/{tenantSlug}/{childSlug}` for sub-surfaces, and public `/{id}` or `/{slug}` where the slug **301-redirects to the id URL** (id is canonical).
- Middleware/extractors: `resolve_tenant` (numeric = id, else slug, else 404), `canonical_tenant`, `ensure_tenant_access` (user must be in `tenant_user`; suspended tenant returns `TENANT_INACTIVE`).
- `TenantScoped` SeaORM helper so tenant-owned queries always filter by `tenant_id`; `make:resource --tenant` adds the column, index and scoping.
- Reserved slug list (`admin`, `api`, `login`, ...). Route helper `tenant_route(name, &tenant)`.
- Role-permission is platform-level (staff). Per-tenant access lives in `tenant_user`. API devices/tokens store the tenant. Storage paths and cache keys are prefixed by tenant.

## Questions to ask me first (one batch)
1. Ship offline sync endpoints, or only plain REST?
2. Canonical tenant URL: id-based for both management and public pages, or id only for public pages?
