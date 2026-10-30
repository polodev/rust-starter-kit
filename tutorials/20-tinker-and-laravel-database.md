<!-- nav:start -->
**Tutorial 20 of 21** | [Index](00-index.md) | [Previous: 19 Components and the contact form](19-components-and-contact-form.md) | [Next: 21 Conventions](21-conventions.md)

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
12. [Cheat sheet and what to build next](12-cheatsheet.md)  
13. [Console commands and the scheduler](13-console-and-schedule.md)  
14. [Login, register and sessions (`auth` module)](14-auth-and-sessions.md)  
15. [Roles and permissions (`role-permission` module)](15-roles-and-permissions.md)  
16. [The admin dashboard (`admin-layout` module)](16-admin-dashboard.md)  
17. [Media library (`media` module)](17-media-library.md)  
18. [JSON API and Google login](18-json-api-and-google-login.md)  
19. [Components and the contact form](19-components-and-contact-form.md)  
**20. [Tinker and sharing the database with Laravel](20-tinker-and-laravel-database.md)**  
21. [Conventions](21-conventions.md)  

</details>
<!-- nav:end -->

# 20. Tinker and sharing the database with Laravel

## Two consoles

Rust is compiled, so it cannot evaluate Rust as you type. Two tasks cover `php artisan tinker`:

| Command | What it is |
|---|---|
| `cargo run -- task tinker` | Built-in **SQL console** |
| `cargo run -- task tinker:eloquent` | **Eloquent and the `DB` facade** on this database (module `rust-eloquent-tinker`; needs PHP 8.2+ and Composer) |

**SQL console.** End SQL with `;`. Dot commands: `.tables`, `.desc users`, `.count users`, `.hash secret`, `.begin`, `.commit`, `.rollback`, `.help`, `.exit`. Use `.begin` to try a change; nothing is saved until `.commit`. `UPDATE` or `DELETE` without `WHERE` asks first. In production it is read-only unless you add `write:true`.

```text
tinker> .begin
tinker*> update users set name = 'Ada' where id = 1;
1 row(s) affected (not saved yet: .commit or .rollback)
tinker*> .rollback
```

**Eloquent tinker.** The first run installs the PHP packages. Then:

```php
User::where('email', 'demo@gmail.com')->first();
User::first()->roles->pluck('name');
Page::with('translations')->get();
DB::table('users')->where('id', 1)->update(['name' => 'Ada']);
DB::transaction(fn () => /* ... */);
```

Or one expression: `cargo run -- task tinker:eloquent run:"User::count()"`. In production add `confirm:true`. Models are in `modules/rust-eloquent-tinker/php/models/`.

## Gotchas when two stacks share a database

- **Passwords are Argon2id**, not bcrypt. A bcrypt hash cannot log in to this app. Assigning `password` on the tinker `User` hashes it correctly; otherwise `rust_password('secret')`.
- **`users.pid`** is 16 raw bytes. The tinker model fills it; `$user->public_id` shows it as text.
- **`media`** is Spatie's table, unchanged (tutorial 17). Files are at `{id}/{file_name}`.
- **Unsigned columns** map to unsigned Rust types.
- **Migrations:** write them with `CREATE TABLE IF NOT EXISTS` when Laravel may already own the table, and keep one migration file per table.
- **Not in tinker:** the rest of Laravel (`Cache`, `Auth`, `Mail`, queues, `route()`).

## Eloquent in Rust code?

Eloquent is PHP and cannot run inside the Rust app. SeaORM is the equivalent:

| Eloquent | SeaORM |
|---|---|
| `User::find(1)` | `users::Entity::find_by_id(1).one(&db).await?` |
| `User::where('email', $e)->first()` | `users::Entity::find().filter(users::Column::Email.eq(e)).one(&db).await?` |
| `->orderBy('id','desc')->limit(10)->get()` | `.order_by_desc(Column::Id).limit(10).all(&db).await?` |
| `User::create([...])` | `users::ActiveModel { name: Set(..), ..Default::default() }.insert(&db).await?` |
| `DB::transaction(...)` | `let txn = db.begin().await?; ...; txn.commit().await?` |

<!-- foot:start -->
---
[Previous: 19 Components and the contact form](19-components-and-contact-form.md) | [Index](00-index.md) | [Next: 21 Conventions](21-conventions.md)
<!-- foot:end -->
