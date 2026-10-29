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

Next: [11-assets-and-styling.md](11-assets-and-styling.md)
