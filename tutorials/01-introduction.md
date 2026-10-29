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

Admin layout, role and permission, a versioned JSON API, login sessions, tenant URLs like Dokani, and more generators. The plan lives in `docs/rust-starter-kit-ideas.md`.

Next: [02-setup.md](02-setup.md)
