# Agent guide for this Loco app

This is a [Loco](https://loco.rs) app — **Rails for Rust**. When you are unsure
how something should work here, the answer is almost always "the way Rails does
it." Where Loco diverges, it is because Rust forced it.

## Read this first

A complete Loco skill ships with this app at **`.claude/skills/loco/`**, matched
to the exact `loco-rs` version in `Cargo.toml`:

| File | What it gives you |
|---|---|
| `.claude/skills/loco/SKILL.md` | start here — the router, `AppContext`, project layout, CLI |
| `.claude/skills/loco/doctrine.md` | what good Loco code looks like; read before writing any |
| `.claude/skills/loco/api-index.md` | every public `loco_rs` symbol, generated from rustdoc — **check here before guessing an API name** |
| `.claude/skills/loco/recipes/` | how to add a model, endpoint, worker, task, mailer, middleware, auth, tests |

If your tool supports Agent Skills, it will load `SKILL.md` automatically. If
not, read it directly — it is a normal markdown file.

## This project's conventions (read before you change anything)

**[`docs/CONVENTIONS.md`](docs/CONVENTIONS.md)** is the rulebook for this starter pack: how modules are laid
out, URL and route shapes, permissions, admin screens (breadcrumb, one container width, ID as the first
clickable column), translations, forms and "component" updates without a reload, the JSON API shape, database
and Laravel-compatibility rules, the media library, security, tests, docs and git. It also has checklists for a
new admin resource, public form, API endpoint and module. Follow it, and update it in the same change when you
introduce a new convention.

Other places to look:

| File | What it gives you |
|---|---|
| `README.md` | run it, the module table, consoles (`tinker`, `tinker:eloquent`) |
| `tutorials/` | numbered walkthroughs (start at `tutorials/00-index.md`) |
| `modules/<name>/README.md` | what one module does, its routes and settings |

Project rules that differ from a stock Loco app: features are `modules/*` crates made with `cargo modular`;
every table has exactly one base migration file in its module (`tests/migrations.rs` checks it), so change the table's own migration and run `cargo run -- db reset`, while someone who has cloned the starter adds new files for their own changes; all visible text is a Fluent key in both
`en-US` and `bn-BD`; the API is under `/api/v1`; run **one** `cargo test` at a time (shared test database).

## The three rules that prevent most mistakes

1. **Generate, then edit.** `cargo loco generate <thing>` writes the file *and*
   the wiring. Rust has no autoloading; hand-wiring is how "the handler exists
   but 404s" happens.
2. **Use the batteries.** This app already has an ORM, queue, scheduler, mailer,
   task runner, storage, cache, and test harness. Adding a crate for something
   Loco already does is the most common mistake.
3. **Fat model, slim controller.** Domain logic on the model; handlers parse,
   call a model method, and render.

## Before you call it done

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

Then check the list for your kind of change in `docs/CONVENTIONS.md` (new admin resource, public form, API
endpoint, module), and update the module `README.md` and the matching tutorial.

## More

- Docs: <https://loco.rs/docs/>
- Framework agent guide: <https://loco.rs/AGENTS.md>
