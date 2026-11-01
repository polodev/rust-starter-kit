# loco-database

One codebase for MySQL/MariaDB **or** PostgreSQL, chosen when the app is built.

| | MySQL / MariaDB (default) | PostgreSQL (`--features postgres`) |
|---|---|---|
| `UnsignedBigInt` | `u64` (`bigint unsigned`) | `i64` (`bigint`) |
| `UnsignedInt` | `u32` (`int unsigned`) | `i32` (`integer`) |
| `UnsignedSmallInt` | `u16` (`smallint unsigned`) | `i32` (`integer`) |
| `UnsignedTinyInt` | `u8` (`tinyint unsigned`) | `i16` (`smallint`) |

Why a build-time choice: a Laravel MySQL table has `bigint unsigned` columns and a signed Rust integer cannot read
them; PostgreSQL has no unsigned integers and SeaORM refuses unsigned Rust types there. The PostgreSQL types match what
Laravel itself creates on PostgreSQL, so a Laravel dump of either kind imports as is.

## Use

- **Entities** use the aliases for unsigned columns: `pub id: UnsignedBigInt`. Convert with `big_int(u64)`, `int(u32)`,
  `small_int(u16)`, `tiny_int(u8)` and back with `to_u64`, `to_u32`, `to_u16`, `to_u8`.
- **Raw SQL migrations** give both versions: `loco_database::run_ddl(manager, MYSQL_SQL, &POSTGRES_STATEMENTS).await`
  (PostgreSQL gets one statement per element, indexes included). `drop_table_if_exists` drops without quoting.
- **Search boxes** use `contains_ignoring_case(Column, text)`: plain `LIKE` ignores case on MySQL but not on PostgreSQL.
- **Start-up** calls `check_backend`, so a build for one database refuses a `DATABASE_URL` for the other, with a message
  that says what to change.

## Build and test

```sh
cargo run                                     # MySQL / MariaDB: DATABASE_URL=mysql://...
cargo run --features postgres                 # PostgreSQL:       DATABASE_URL=postgres://...
cargo test --workspace                        # MySQL / MariaDB
TEST_DATABASE_URL=postgres://user@127.0.0.1:5432/rust_starter_test \
  cargo test --workspace --features loco-database/postgres --target-dir target-pg   # PostgreSQL
```

Use a separate `--target-dir` for the PostgreSQL build so switching does not recompile everything each time.
