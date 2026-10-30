<!-- nav:start -->
**Tutorial 3 of 25** | [Index](00-index.md) | [Previous: 2 Setup](02-setup.md) | [Next: 4 Project structure](04-project-structure.md)

<details><summary>Jump to any tutorial</summary>

[00. Index](00-index.md)  
01. [Introduction](01-introduction.md)  
02. [Setup](02-setup.md)  
**03. [Rust essentials](03-rust-essentials.md)**  
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

</details>
<!-- nav:end -->

# 03. Rust essentials

Only what you need to read and change this project. Every example is from, or close to, the real code.

## Variables and types

```rust
let name = "Rust";              // immutable by default
let mut count = 0;              // `mut` makes it changeable
count += 1;
let slug: String = "about".to_string();   // an owned, growable string
let code: &str = "bn";                    // a borrowed string slice
```

`String` owns its text. `&str` only looks at text owned elsewhere. Most function parameters take `&str`.

## Ownership and borrowing

Every value has one owner. When the owner goes away, the value is freed. There is no garbage collector.

```rust
let a = String::from("hi");
let b = a;          // ownership MOVES to b; `a` can no longer be used
let c = &b;         // `c` borrows b (read only)
```

- `&T` is a shared, read-only borrow. You can have many.
- `&mut T` is an exclusive, changeable borrow. You can have only one at a time.

The compiler checks this. Its error messages are long but accurate: read them from the top.

## Structs, enums, `impl`

```rust
pub struct Locale(pub &'static str);        // tuple struct

impl Locale {
    pub fn code(self) -> &'static str { self.0 }
}

enum Status { Draft, Published }
```

## `Option` and `Result`: no null, no exceptions

```rust
fn normalize(code: &str) -> Option<&'static str> { /* Some("en") or None */ }
fn find_page() -> Result<PageView, Error>       { /* Ok(page) or Err(e) */ }
```

Handling them:

```rust
match normalize(input) {
    Some(code) => println!("{code}"),
    None => println!("unsupported"),
}

let code = normalize(input).unwrap_or("en");     // default value
let page = find_page()?;                         // `?` returns the error early
if let Some(code) = normalize(input) { /* ... */ }
```

`?` is the everyday tool. It only works in a function that itself returns a `Result`. Avoid `.unwrap()` in app code: it panics. It is fine in tests.

## Traits (like interfaces)

```rust
#[async_trait]
pub trait Module: Send + Sync {
    fn name(&self) -> &'static str;
    fn routes(&self) -> Vec<Routes> { Vec::new() }   // default method
}

impl Module for PagesModule {
    fn name(&self) -> &'static str { "pages" }
}
```

`Vec<Box<dyn Module>>` is a list of different types that all implement `Module`. This is how the module registry works.

## Closures and iterators

```rust
let codes: Vec<&str> = pairs.iter().map(|p| p.code).collect();
let found = list.iter().find(|m| m.name() == "pages");
```

`|x| expression` is an anonymous function.

## Async

Web servers wait on the network and database. `async fn` returns a future, and `.await` pauses until it is ready.

```rust
async fn show(State(ctx): State<AppContext>) -> Result<Response> {
    let page = pages::Entity::find().one(&ctx.db).await?;
    // ...
}
```

You can only `.await` inside an `async fn`. The runtime (Tokio) runs them.

## Modules and crates

- A **crate** is a package with a `Cargo.toml` (a library or a program).
- A **module** is a file or folder inside a crate: `mod controllers;` loads `controllers.rs` or `controllers/mod.rs`.
- `pub` makes something visible outside its module. Default is private.
- `use module_site_layout::Locale;` brings a name into scope.

## Attributes and macros

Lines starting with `#[...]` add behaviour:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]   // generate code automatically
struct PageView { title: String }

#[debug_handler]                                   // better error messages for Axum handlers
async fn show() { }
```

`name!(...)` calls a macro, for example `format!("{a}{b}")` or `data!({ "lang": "bn" })`.

## Cargo in five commands

| Command | Meaning |
|---|---|
| `cargo build` | Compile |
| `cargo run -- start` | Run the app; everything after `--` goes to the app |
| `cargo test` | Run tests |
| `cargo check` | Type-check without producing a binary (fast) |
| `cargo fmt` / `cargo clippy` | Format / lint |

## Reading compiler errors

1. Read the first error only. Later ones are often caused by it.
2. Look for `help:` lines. They often contain the exact fix.
3. `cannot borrow ... as mutable` or `value moved` means an ownership problem: clone (`.clone()`), borrow (`&`), or restructure.

<!-- foot:start -->
---
[Previous: 2 Setup](02-setup.md) | [Index](00-index.md) | [Next: 4 Project structure](04-project-structure.md)
<!-- foot:end -->
