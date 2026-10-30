<!-- nav:start -->
**Tutorial 12 of 25** | [Index](00-index.md) | [Previous: 11 Assets and styling](11-assets-and-styling.md) | [Next: 13 Console commands and the scheduler](13-console-and-schedule.md)

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
**12. [Cheat sheet and what to build next](12-cheatsheet.md)**  
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

# 12. Cheat sheet and what to build next

## Commands

| Task | Command |
|---|---|
| Start the server | `cargo run -- start` |
| Migrate | `cargo run -- db migrate` |
| Seed | `cargo run -- db seed` |
| List routes | `cargo run -- routes` |
| Run a task | `cargo run -- task <name>` |
| New module | `cargo modular make:module <name>` |
| New command | `cargo modular make:command <Name> --module=<m>` |
| List / run commands | `cargo modular command:list` / `command:run <sig> k:v` |
| Schedule | `cargo modular schedule:list` / `schedule:work` |
| List modules | `cargo modular modules:list` |
| Tests | `cargo test --workspace` |
| Format and lint | `cargo fmt --all` and `cargo clippy --workspace --all-targets` |

## Laravel to Rust

| Laravel | Rust here |
|---|---|
| `Route::get('/x', [C::class, 'show'])` | `Routes::new().add("/x", get(show))` |
| Form Request | A struct with `#[derive(Deserialize, Validate)]` as an extractor |
| `Model::where('slug', $s)->first()` | `Entity::find().filter(Column::Slug.eq(s)).one(db).await?` |
| `$model->save()` | `active_model.insert(db).await?` or `.update(db)` |
| `dd($x)` | `dbg!(&x)` or `tracing::debug!(?x)` |
| `config('app.name')` | `ctx.config` or an environment variable in `config/*.yaml` |
| `Cache::remember` | `loco_modular::remember` |
| `abort(404)` | `Err(Error::NotFound)` or an explicit `404` response |
| Blade `@extends`, `@section` | Tera `{% extends %}`, `{% block %}` |
| `__('key')` | `{{ t(key="...", lang=tag) }}` |
| Service provider | `impl Module for X` and `App`'s `Hooks` |
| `.env` | `.env` plus `config/*.yaml` |

## Common mistakes

| Mistake | Fix |
|---|---|
| New module not found | `cargo modular modules:sync`, then rebuild |
| Template not found | Add it to the module's `templates()` list |
| Page unstyled after a new class | Rebuild the Tailwind CSS |
| A translation shows the key | Add it to **both** `.ftl` files |
| Test database refused | `TEST_DATABASE_URL` must end in `_test` |
| `unwrap()` panic in a handler | Use `?` or return a proper error |

## Built, and what to build next

**Built:** the `cargo modular` generators, admin shell and sidebar, login sessions, roles and permissions, a bearer-token JSON API, Google sign-in, a Spatie-compatible media library, a contact form with an admin inbox, an activity log, a queue monitor, database backups, and the SQL and Eloquent consoles. The README checklist is the full list.

**Next, in this order:**

1. **More generators**: `cargo modular make:model`, `make:controller`, `make:scaffold` with `--module`, and `make:resource` for admin CRUD.
2. **Media on R2**: public collections to a public bucket, private ones to a private bucket (the keys are already in `.env`).
3. **Settings, menus and a blog** module, plus email verification and password reset.
4. **Tenant URLs:** `/business/{tenant}/...`, with the slug redirecting to the id URL.
5. **Image conversions** (thumbnails), sitemap and SEO tags, OpenAPI documentation.

<!-- foot:start -->
---
[Previous: 11 Assets and styling](11-assets-and-styling.md) | [Index](00-index.md) | [Next: 13 Console commands and the scheduler](13-console-and-schedule.md)
<!-- foot:end -->
