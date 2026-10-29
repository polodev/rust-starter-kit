# 05. Modules and `cargo modular`

A module is a self-contained feature in `modules/<name>`, like an `app-modules/` package in Laravel.

## Commands

```sh
cargo modular make:module blog       # create modules/blog and register it
cargo modular make:module blog --dry-run
cargo modular modules:list           # shows registered / NOT registered
cargo modular modules:sync           # rebuild the registry from modules/
```

`cargo modular` is a Cargo alias in `.cargo/config.toml` that runs the `cargo-modular` crate. Names are normalised: `Role Permission` becomes `role-permission`.

## What `make:module blog` does

1. Creates the crate:
   ```
   modules/blog/
     Cargo.toml
     src/lib.rs                 BlogModule
     src/controllers/mod.rs     routes()
     src/migrations/mod.rs      all()
     src/commands/mod.rs        register() for console commands
     templates/                 Tera templates
     tests/
   ```
2. Adds `modules/*` to the workspace members (first module only).
3. Adds `module-blog` to `packages/modules-registry/Cargo.toml`.
4. Rewrites `packages/modules-registry/src/lib.rs`.

Never edit the registry by hand. Run `modules:sync` instead.

## The `Module` trait

Every module implements `loco_modular::Module`. All methods except `name` are optional:

```rust
pub struct PagesModule;

#[async_trait]
impl Module for PagesModule {
    fn name(&self) -> &'static str { "pages" }

    fn routes(&self) -> Vec<Routes> { controllers::routes() }
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> { migrations::all() }
    fn templates(&self) -> Vec<(&'static str, &'static str)> {
        vec![("pages/show.html", include_str!("../templates/pages/show.html"))]
    }
    async fn seed(&self, ctx: &AppContext) -> Result<()> { seeds::run(&ctx.db).await }
}
```

| Method | Wired into |
|---|---|
| `routes` | `App::routes` |
| `migrations` | `Migrator` (merged and sorted by name) |
| `templates` | The Tera engine at boot |
| `register_tasks` | `cargo run -- task <name>` (console commands, see [13](13-console-and-schedule.md)) |
| `schedule` | The scheduler (`cargo modular schedule:list`) |
| `connect_workers` | Background queue |
| `seed` | `cargo run -- db seed` |

`include_str!` embeds the template file into the binary at compile time, so a module is fully self-contained.

## Template names

Module templates share one namespace, so prefix them with the module or area (`pages/show.html`, `site/home.html`, `layouts/site.html`). A module can `{% extends "layouts/site.html" %}` from another module.

## Adding a feature: the usual steps

1. `cargo modular make:module blog`
2. Add a migration and an entity (see [07-database.md](07-database.md)).
3. Add a controller and routes (see [06](06-routes-controllers-views.md)).
4. Add templates and list them in `templates()`.
5. `cargo run -- db migrate` and test it.

## Gotcha

If `cargo` says `failed to read modules/*/Cargo.toml`, the workspace lists `modules/*` but no module exists yet. Create one with `cargo modular make:module` (it adds the glob only when needed).

Next: [06-routes-controllers-views.md](06-routes-controllers-views.md)
