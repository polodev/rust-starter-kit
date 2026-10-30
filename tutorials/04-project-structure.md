<!-- nav:start -->
**Tutorial 4 of 25** | [Index](00-index.md) | [Previous: 3 Rust essentials](03-rust-essentials.md) | [Next: 5 Modules and `cargo modular`](05-modules.md)

<details><summary>Jump to any tutorial</summary>

[00. Index](00-index.md)  
01. [Introduction](01-introduction.md)  
02. [Setup](02-setup.md)  
03. [Rust essentials](03-rust-essentials.md)  
**04. [Project structure](04-project-structure.md)**  
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

</details>
<!-- nav:end -->

# 04. Project structure

```
rust-starter-pack/
  Cargo.toml                 workspace + the app crate
  src/                       the Loco app (thin)
    app.rs                   Hooks: routes, tasks, workers, seed
    initializers/            view engine setup
    controllers/ models/ ... Loco defaults (auth, users)
  migration/                 the migrator: app migrations + every module's
  config/                    development.yaml, test.yaml, production.yaml
  assets/
    i18n/en-US, bn-BD/       translation strings (Fluent)
    css/input.css            Tailwind input
    static/                  served at /static (built CSS lives here)
  modules/                   feature crates (created by cargo modular)
    site-layout/ auth/ auth-api/ social-auth/ role-permission/ admin-layout/ accounts/
    pages/ media/ contact/ activity-log/ queue-watcher/ db-backup/ rust-eloquent-tinker/
  packages/
    cargo-modular/           the CLI
    loco-modular/            the Module trait + helpers
    loco-activity-log/       the activity-log writer, so any module can log
    modules-registry/        generated list of modules
  tests/                     integration tests
  tutorials/  docs/
```

## Workspace

The root `Cargo.toml` starts with `[workspace]`. A workspace is several crates built together with one `target/` folder and one `Cargo.lock`.

`[workspace.dependencies]` pins versions once. Member crates then write `loco-rs = { workspace = true }`, like a shared `composer.json` constraint.

## Who depends on whom

```
rust-starter (app)  ──►  modules-registry  ──►  module-pages, module-site-layout
        │                                              │
        └──►  migration  ──►  modules-registry         └──►  loco-modular
```

- Modules never depend on the app. The app depends on modules through the registry.
- `migration` also reads the registry, so module tables migrate with the rest.
- Rust forbids circular dependencies, which is why the registry is its own crate.

## What `Hooks` does (`src/app.rs`)

`App` implements Loco's `Hooks` trait. The important methods:

| Method | Job |
|---|---|
| `routes` | Builds all routes: Loco defaults, auth, then each module's routes |
| `initializers` | Registers the view engine (templates and translations) |
| `connect_workers` | Registers background workers, including each module's |
| `register_tasks` | Registers CLI tasks, including each module's |
| `seed` | Runs every module's seeder |
| `boot` | Starts the app. Here it also refuses a non-`_test` database in the test environment |

## Config

`config/*.yaml` is chosen by the environment (`LOCO_ENV`, default `development`). Values like `<%= get_env(name="DATABASE_URL", default="...") %>` read environment variables with a fallback.

<!-- foot:start -->
---
[Previous: 3 Rust essentials](03-rust-essentials.md) | [Index](00-index.md) | [Next: 5 Modules and `cargo modular`](05-modules.md)
<!-- foot:end -->
