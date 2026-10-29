<!-- nav:start -->
**Tutorial 10 of 17** | [Index](00-index.md) | [Previous: 09 Cache and sessions](09-cache-and-sessions.md) | [Next: 11 Assets and styling](11-assets-and-styling.md)

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
**10. [Testing](10-testing.md)**  
11. [Assets and styling](11-assets-and-styling.md)  
12. [Cheat sheet and what to build next](12-cheatsheet.md)  
13. [Console commands and the scheduler](13-console-and-schedule.md)  
14. [Login, register and sessions (`auth` module)](14-auth-and-sessions.md)  
15. [Roles and permissions (`role-permission` module)](15-roles-and-permissions.md)  
16. [The admin dashboard (`admin-layout` module)](16-admin-dashboard.md)  
17. [Media library (`media` module)](17-media-library.md)  

</details>
<!-- nav:end -->

# 10. Testing

```sh
cargo test --workspace          # everything
cargo test --test mod pages     # integration tests whose name contains "pages"
cargo test -p cargo-modular     # one crate
```

## The safety rule

The test environment **truncates and recreates its schema** on every run. So tests may only use a database whose name ends in `_test`. `App::boot` refuses anything else:

```
refusing to boot the test environment: the database name must end in `_test`
```

Tests read `TEST_DATABASE_URL`, not `DATABASE_URL`, so a normal `.env` can never point tests at your development data. Do not weaken this guard.

## Three kinds of tests

**Unit tests** live in the same file as the code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_language_uses_quality_order() {
        assert_eq!(from_accept_language(Some("fr;q=0.9, bn-BD;q=0.8")), Some("bn"));
    }
}
```

**Integration tests** in `tests/` start the whole app and send real requests:

```rust
#[tokio::test]
#[serial]
async fn unknown_page_is_a_translated_404() {
    seeded(|server| async move {
        let response = server.get("/pages/missing").add_header("cookie", "lang=bn").await;
        response.assert_status_not_found();
        assert!(response.text().contains("পৃষ্ঠাটি পাওয়া যায়নি"));
    })
    .await;
}
```

**Template tests** render a view directly (`tests/views/render.rs`).

## Useful pieces

- `#[serial]` (serial_test) runs tests one at a time. Needed because they share one database.
- `#[rstest]` with `#[case(...)]` runs one test with many inputs. Used for the redirect-safety cases.
- `request::<App, _, _>(|server, ctx| async move { ... })` boots the app and gives you an HTTP client and the app context.
- `seed::<App>(&ctx)` loads fixtures (test users) and runs module seeders.
- Prefer the specific assertion helpers (`assert_status_ok`, `assert_status_not_found`) over comparing numbers.

## What to test for every feature

1. The happy path.
2. The failure paths: missing record, invalid input, wrong language.
3. The unusual paths: running the seeder twice, hostile input such as `next=//evil.example`.

## Before you say it is done

```sh
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

<!-- foot:start -->
---
[Previous: 09 Cache and sessions](09-cache-and-sessions.md) | [Index](00-index.md) | [Next: 11 Assets and styling](11-assets-and-styling.md)
<!-- foot:end -->
