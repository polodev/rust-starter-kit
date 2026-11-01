<!-- nav:start -->
**Tutorial 1 of 28** | [Index](00-index.md) | Previous: none | [Next: 2 Setup](02-setup.md)

<details><summary>Jump to any tutorial</summary>

[00. Index](00-index.md)  
**01. [Introduction](01-introduction.md)**  
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
21. [Conventions](21-conventions.md)  
22. [Activity log, queue monitor and database backups](22-activity-log-queue-and-backups.md)  
23. [PDF and email](23-pdf-and-email.md)  
24. [Email and PDF step by step](24-email-and-pdf-step-by-step.md)  
25. [AI service: Claude, OpenAI and Gemini](25-ai-service.md)  
26. [Commands and jobs for Laravel developers](26-commands-and-jobs-for-laravel-developers.md)  
27. [PostgreSQL for MySQL users](27-postgresql-for-mysql-users.md)  
28. [MySQL or PostgreSQL, one choice per app](28-mysql-or-postgresql-per-app.md)  

</details>
<!-- nav:end -->

# 01. Introduction

## What this is

A starter kit for building websites in Rust with the speed of working in Laravel.

| Layer | Choice |
|---|---|
| Framework | [Loco](https://loco.rs), built on Axum |
| Database | MariaDB through SeaORM |
| Views | Tera templates (Loco's built-in engine) |
| Styling | Tailwind CSS (standalone CLI, no Node) |
| Interactivity | HTMX and Alpine.js (planned for the admin) |
| Languages | English and Bangla |
| Feature packaging | Module crates, like `app-modules/` in Laravel |

## How it maps to Laravel

| Laravel | Here |
|---|---|
| `composer.json` | `Cargo.toml` |
| `app-modules/blog` (internachi/modular) | `modules/blog` crate |
| `php artisan make:module blog` | `cargo modular make:module blog` |
| `php artisan migrate` | `cargo run -- db migrate` |
| `php artisan db:seed` | `cargo run -- db seed` |
| `php artisan serve` | `cargo run -- start` |
| Eloquent model | SeaORM entity |
| Blade | Tera |
| Livewire | HTMX (server returns HTML fragments) |
| `Cache::remember` | `loco_modular::remember` |
| `php artisan make:command` / `schedule:list` | `cargo modular make:command` / `schedule:list` |
| `php artisan test` | `cargo test` |

## What exists today

- `cargo modular` CLI: `make:module`, `make:command`, `modules:list`, `modules:sync`, `command:list`, `command:run`, `schedule:list`, `schedule:work`
- `site-layout` module: public layout, home page, language switching
- `pages` module: About, Privacy Policy and Terms of Service in English and Bangla
- MariaDB database `rust_starter`, test database `rust_starter_test`
- Tests, a test-database guard and a Tailwind build

## What is planned

Admin layout, role and permission, a versioned JSON API, login sessions, tenant URLs, and more generators.

## Inspired by

Rails and Loco (generators, convention over configuration), and Laravel and its ecosystem: `internachi/modular`, Sanctum, Socialite, Livewire/Volt, Eloquent and tinker, and Spatie's media library. The README has the full table. The names are kept where Rust allows it, so a Laravel developer can find things.

<!-- foot:start -->
---
Previous: none | [Index](00-index.md) | [Next: 2 Setup](02-setup.md)
<!-- foot:end -->
