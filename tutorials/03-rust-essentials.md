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

Next: [04-project-structure.md](04-project-structure.md)
