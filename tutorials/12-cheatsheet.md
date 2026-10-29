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

## What to build next

In this order:

1. **More generators** that follow Loco: `cargo modular make:model`, `make:controller`, `make:scaffold` with `--module`, and a `make:resource` for admin CRUD.
2. **admin-layout** module: sidebar built from a registry, topbar, flash messages, HTMX helpers, Tailwind.
3. **Login sessions**: cookie session stored in the database, with the language preference saved on the user.
4. **role-permission** module, ported from Dokani: config-defined roles with inheritance, wildcard permissions, fail-closed admin routes.
5. **api** module under `/api/v1`: response envelope, device tokens, rate limits, request log.
6. **Tenant URLs** like Dokani: `/business/{tenant}/...`, with the slug redirecting to the id URL.
7. **Storage** for uploads on R2, settings, activity log, queue and scheduler pages.

The full plan is in `docs/rust-starter-kit-ideas.md`.
