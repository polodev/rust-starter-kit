<!-- nav:start -->
**Tutorial 7 of 21** | [Index](00-index.md) | [Previous: 6 Routes, controllers and views](06-routes-controllers-views.md) | [Next: 8 A bilingual site (English and Bangla)](08-bilingual-site.md)

<details><summary>Jump to any tutorial</summary>

[00. Index](00-index.md)  
01. [Introduction](01-introduction.md)  
02. [Setup](02-setup.md)  
03. [Rust essentials](03-rust-essentials.md)  
04. [Project structure](04-project-structure.md)  
05. [Modules and `cargo modular`](05-modules.md)  
06. [Routes, controllers and views](06-routes-controllers-views.md)  
**07. [Database: migrations, entities, queries, seeding](07-database.md)**  
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

</details>
<!-- nav:end -->

# 07. Database: migrations, entities, queries, seeding

SeaORM talks to MariaDB. Three pieces: a **migration** creates the table, an **entity** maps it to a Rust struct, and **queries** use the entity.

## Migrations

`modules/pages/src/migrations/m20261030_000005_create_pages.rs`:

```rust
create_table(
    m,
    "pages",
    &[
        ("id", ColType::PkAuto),
        ("slug", ColType::StringLenUniq(120)),
        ("is_published", ColType::BooleanWithDefault(true)),
        ("sort_order", ColType::IntegerWithDefault(0)),
    ],
    &[],
).await?;
```

- `create_table` comes from `loco_rs::schema`. It adds `created_at` and `updated_at` automatically.
- `refs: &[("pages", "")]` adds a `page_id` column with a foreign key to `pages.id`.
- Register the migration in the module's `migrations/mod.rs`. File names start with a timestamp; migrations from all modules run in name order.
- Write the `down` method too, so you can roll back locally.

Run them:

```sh
cargo run -- db migrate
```

**One file per table.** The starter ships one base migration for each table and no `ALTER` files, so a fresh clone sees a clean schema. To change a table of the starter itself, edit its migration and rebuild your local database: `cargo run -- db reset` then `cargo run -- db seed`. Once your own project has data you care about (a shared or production database), switch to forward migrations: add a **new** file for each change and never edit one that has run. `tests/migrations.rs` keeps the starter honest.

## Entities

```rust
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "pages")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub slug: String,
    pub is_published: bool,
    pub sort_order: i32,
}
```

`Model` is a row. The derive macro also creates `Entity`, `Column` and `ActiveModel`. Relations are declared in a `Relation` enum (see `modules/pages/src/entities/`).

## Queries

```rust
let page = pages::Entity::find()
    .filter(pages::Column::Slug.eq(slug))
    .filter(pages::Column::IsPublished.eq(true))
    .one(db)          // Option<Model>
    .await?;

let all = pages::Entity::find().order_by_asc(pages::Column::SortOrder).all(db).await?;
```

Inserting uses an `ActiveModel`; fields you do not set stay unset:

```rust
pages::ActiveModel {
    slug: Set("about".to_string()),
    is_published: Set(true),
    sort_order: Set(1),
    ..Default::default()
}
.insert(db)
.await?;
```

To avoid the N+1 problem, load related rows in one query with `find_related` or `find_with_related` instead of querying inside a loop.

## Seeding

A module's `seed` method must be **idempotent**: safe to run many times. The pages seeder inserts only what is missing and never overwrites later edits:

```rust
let page = match pages::Entity::find().filter(pages::Column::Slug.eq(seed.slug)).one(db).await? {
    Some(page) => page,
    None => pages::ActiveModel { /* ... */ }.insert(db).await?,
};
```

Run `cargo run -- db seed`. Running it twice leaves the same 3 pages and 6 translations.

## MariaDB notes

- Use `utf8mb4` (already set on the databases).
- Loco's own YAML `db::seed` helper does not support MariaDB. Use `loco_modular::seed_yaml` for fixture files.
- `Text` columns are fine for page bodies (up to 64 KB). Use a longer type if you expect more.
- Queue storage is not in the database here; background work uses Loco's async workers.

<!-- foot:start -->
---
[Previous: 6 Routes, controllers and views](06-routes-controllers-views.md) | [Index](00-index.md) | [Next: 8 A bilingual site (English and Bangla)](08-bilingual-site.md)
<!-- foot:end -->
