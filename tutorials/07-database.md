# 07. Database: migrations, entities, queries, seeding

SeaORM talks to MariaDB. Three pieces: a **migration** creates the table, an **entity** maps it to a Rust struct, and **queries** use the entity.

## Migrations

`modules/pages/src/migrations/m20261030_000001_create_pages.rs`:

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

**Never** drop, reset or truncate the development database for convenience. Use forward migrations only.

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

Next: [08-bilingual-site.md](08-bilingual-site.md)
