# Todo queue

Every instruction from the conversation, in the order it will be done. Status: `[x]` done, `[~]` in progress, `[ ]` waiting.
Update this file as work finishes.

## Done
- [x] `cargo modular` CLI: make:module, make:command, modules:list/sync, command:list/run, schedule:list/work
- [x] Loco app on MariaDB `rust_starter` (+ `rust_starter_test`), R2 config tested, test-database guard
- [x] `site-layout` + `pages` (About, Privacy Policy, Terms of Service in English and Bangla)
- [x] Language dropdown in eng4 style; dark/light theme toggle
- [x] Demo user seeder `demo@gmail.com` (developer role assignment is queued below)
- [x] Console commands, schedules, scheduler run
- [x] Tutorials 01-17 with `00-index.md` and a navigation bar on every page
- [x] Module hooks: admin menu, apply, schedule (loco-modular)
- [x] `role-permission` module: 6-role hierarchy new_employee < business_development < marketing < employee < admin < developer; developer holds every permission incl. the role manager; `dashboard.view` = "View admin dashboard"

- [x] Public GitHub repo https://github.com/polodev/rust-starter-kit (secret scan clean, `.env` ignored)

## Queue (do in this order)
0. [x] Media like Laravel: files belong to models. Pages have a feature image (single-file collection) edited in `/admin/pages/{id}/edit` and shown on the public page; `/admin/media` browses every file with owner links
1. [x] Navbar responsive, exactly like Dokani (navy bar, EN switch, theme dropdown, amber avatar, hamburger)
2. [x] `auth` module: login, register, logout, DB sessions, CSRF origin check, login lockout (15 tests)
3. [x] App wired: module `apply` initializer, admin menu, storage disk, demo user is `developer`
4. [x] `admin-layout`: eng4-style sidebar with permission-filtered sections and Ctrl/Cmd+K quick search; dashboard like Dokani; templates for dashboard, users (assign role), roles matrix, scheduled jobs
5. [x] Header dropdown: admin link for `dashboard.view`, profile page, logout
6. [x] `media` module like Spatie MediaLibrary: single-file and multiple-file collections, `HasMedia` trait on models, easy image upload, admin media library page (`media.manage`)
7. [x] `accounts` module: profile page, change password, logo upload (uses media), sub-layout with logout
8. [x] Tests for auth, roles, admin, media, accounts; tutorials 14-17
9. [ ] Answer: hot reload (needs `cargo-watch`; module templates are compiled in, so a dev-mode read-from-disk option may help)
10. [x] Dashboard page shows every sidebar link as compact chips with search, like eng4 (Bimafy's `/dashboard/links` does the same by scraping the sidebar)
11. [ ] Generators that follow Loco: make:model, make:controller, make:scaffold with `--module`, then make:resource
12. [ ] `api` module under `/api/v1`, URL-based tenancy like Dokani, R2 storage use, settings, activity log, queue pages
