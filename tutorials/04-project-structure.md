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
    site-layout/  pages/
  packages/
    cargo-modular/           the CLI
    loco-modular/            the Module trait + helpers
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

Next: [05-modules.md](05-modules.md)
