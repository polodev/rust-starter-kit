<!-- nav:start -->
**Tutorial 21 of 21** | [Index](00-index.md) | [Previous: 20 Tinker and sharing the database with Laravel](20-tinker-and-laravel-database.md) | Next: none

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
**21. [Conventions](21-conventions.md)**  

</details>
<!-- nav:end -->

# 21. Conventions

The full rulebook is **[`docs/CONVENTIONS.md`](../docs/CONVENTIONS.md)**. `AGENTS.md` and `CLAUDE.md` point coding agents to it, so people and agents follow the same rules. This page is the short version.

## Ten rules

1. **Generate, then edit.** `cargo modular make:module` / `make:command`. Never hand-wire.
2. **One feature, one module** in `modules/<name>/`, with its own migrations, routes, templates, permissions and tests. No dependency cycles (`admin-layout` cannot depend on `media`).
3. **Routes:** public has no prefix, admin is `/admin`, the API is `/api/v1`. Admin resources are list, show, edit and `POST` for saves.
4. **Fat model, slim controller.** A handler parses, checks the permission, calls a function, renders.
5. **Permissions** are `<area>.view` and `<area>.manage`, defined in `role-permission/src/definitions.rs`, then `roles:sync`.
6. **Admin pages:** one container, a breadcrumb, an ID first column that links to Show, then Actions.
7. **Text:** every visible string is a Fluent key in `en-US` and `bn-BD`. Database content: English required, other languages optional and falling back.
8. **Forms:** POST only, `same_origin` first, redirect after success, 422 with the typed values on error. Public forms get a honeypot and a rate limit.
9. **Database:** migrations live in their module, **one file per table** (no ALTER files; `tests/migrations.rs` checks it); stay compatible with Laravel (`media`, Argon2id, binary `pid`).
10. **Done means:** `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` (one run at a time), plus the README and tutorial updated.

## Before you start something new

Open the matching **checklist** at the end of `docs/CONVENTIONS.md`: new admin resource, public form, API endpoint or module. Add a new rule there when you introduce a new convention.

## Commits

A short imperative subject, then bullets of what and why. One logical change per commit, only after the three checks pass, and no secrets in the diff.

<!-- foot:start -->
---
[Previous: 20 Tinker and sharing the database with Laravel](20-tinker-and-laravel-database.md) | [Index](00-index.md) | Next: none
<!-- foot:end -->
