//! One codebase, two databases. An app is built for MySQL/MariaDB (the default) or for PostgreSQL
//! (`--features postgres`), because the two need different Rust integer types:
//!
//! - MySQL columns made by Laravel are `bigint unsigned`, and a signed Rust integer cannot read them.
//! - PostgreSQL has no unsigned integers, and SeaORM refuses unsigned Rust types on it.
//!
//! So entities use the aliases below. On MySQL they are `u64`, `u32`, ...; with the `postgres` feature they are
//! `i64`, `i32`, ... Migrations written as raw SQL give both versions to [`run_ddl`]. At start-up
//! [`check_backend`] makes sure the build matches `DATABASE_URL`.

// The conversion helpers are identity conversions on one database and widening or narrowing on the other, so
// clippy calls them useless or infallible in whichever build it checks.
#![allow(clippy::useless_conversion, clippy::unnecessary_fallible_conversions)]

use sea_orm::{ConnectionTrait, DatabaseBackend, DbErr};
use sea_orm_migration::SchemaManager;

/// `bigint unsigned` on MySQL (ids of Laravel-style tables), `bigint` on PostgreSQL.
#[cfg(not(feature = "postgres"))]
pub type UnsignedBigInt = u64;
/// `int unsigned` on MySQL, `integer` on PostgreSQL (what Laravel's `unsignedInteger` makes there; unix times fit until 2038).
#[cfg(not(feature = "postgres"))]
pub type UnsignedInt = u32;
/// `smallint unsigned` on MySQL, `integer` on PostgreSQL (an unsigned 16-bit value does not fit `smallint`).
#[cfg(not(feature = "postgres"))]
pub type UnsignedSmallInt = u16;
/// `tinyint unsigned` on MySQL, `smallint` on PostgreSQL.
#[cfg(not(feature = "postgres"))]
pub type UnsignedTinyInt = u8;

#[cfg(feature = "postgres")]
pub type UnsignedBigInt = i64;
#[cfg(feature = "postgres")]
pub type UnsignedInt = i32;
#[cfg(feature = "postgres")]
pub type UnsignedSmallInt = i32;
#[cfg(feature = "postgres")]
pub type UnsignedTinyInt = i16;

/// A `u64` as the database integer type of this build, clamped to its range.
pub fn big_int(value: u64) -> UnsignedBigInt {
    UnsignedBigInt::try_from(value).unwrap_or(UnsignedBigInt::MAX)
}
/// A `u32` as the database integer type of this build, clamped to its range.
pub fn int(value: u32) -> UnsignedInt {
    UnsignedInt::try_from(value).unwrap_or(UnsignedInt::MAX)
}
/// A `u16` as the database integer type of this build, clamped to its range.
pub fn small_int(value: u16) -> UnsignedSmallInt {
    UnsignedSmallInt::try_from(value).unwrap_or(UnsignedSmallInt::MAX)
}
/// A `u8` as the database integer type of this build, clamped to its range.
pub fn tiny_int(value: u8) -> UnsignedTinyInt {
    UnsignedTinyInt::try_from(value).unwrap_or(UnsignedTinyInt::MAX)
}
/// The database value as a `u64` (a negative value, impossible in practice, becomes 0).
pub fn to_u64(value: UnsignedBigInt) -> u64 {
    u64::try_from(value).unwrap_or(0)
}
/// The database value as a `u32`.
pub fn to_u32(value: UnsignedInt) -> u32 {
    u32::try_from(value).unwrap_or(0)
}
/// The database value as a `u16`.
pub fn to_u16(value: UnsignedSmallInt) -> u16 {
    u16::try_from(value).unwrap_or(0)
}
/// The database value as a `u8`.
pub fn to_u8(value: UnsignedTinyInt) -> u8 {
    u8::try_from(value).unwrap_or(0)
}

/// A "contains this text, ignoring case" filter for search boxes. MySQL's usual collations ignore case in `LIKE`;
/// PostgreSQL's `LIKE` does not, so a PostgreSQL build uses `ILIKE`.
pub fn contains_ignoring_case<C: sea_orm::ColumnTrait>(
    column: C,
    text: &str,
) -> sea_orm::sea_query::Expr {
    if cfg!(feature = "postgres") {
        column.ilike(format!("%{text}%"))
    } else {
        column.contains(text)
    }
}

/// The database this build was made for.
pub const BUILT_FOR: DatabaseBackend = if cfg!(feature = "postgres") {
    DatabaseBackend::Postgres
} else {
    DatabaseBackend::MySql
};

/// Fails when the connection is to another kind of database than the build expects, with a message that says
/// what to change. Call it once at start-up.
pub fn check_backend(actual: DatabaseBackend) -> Result<(), String> {
    let normalised = |backend: DatabaseBackend| match backend {
        DatabaseBackend::Postgres => "PostgreSQL",
        DatabaseBackend::MySql => "MySQL/MariaDB",
        _ => "another",
    };
    if actual == BUILT_FOR {
        return Ok(());
    }
    let fix = match BUILT_FOR {
        DatabaseBackend::Postgres => {
            "use a postgres:// DATABASE_URL, or build without --features postgres"
        }
        _ => "use a mysql:// DATABASE_URL, or build with --features postgres",
    };
    Err(format!(
        "this build is for {} but DATABASE_URL is a {} database: {fix}",
        normalised(BUILT_FOR),
        normalised(actual)
    ))
}

/// Runs a migration's raw SQL for the connected database: `mysql` as one statement, or the `postgres`
/// statements one after the other (PostgreSQL cannot run several in one call, and indexes are separate).
pub async fn run_ddl(
    manager: &SchemaManager<'_>,
    mysql: &str,
    postgres: &[&str],
) -> Result<(), DbErr> {
    let db = manager.get_connection();
    match manager.get_database_backend() {
        DatabaseBackend::Postgres => {
            for statement in postgres {
                db.execute_unprepared(statement).await?;
            }
        }
        _ => {
            db.execute_unprepared(mysql).await?;
        }
    }
    Ok(())
}

/// Runs a raw `DROP` for the connected database (the same statement works on both when written without quotes).
pub async fn drop_table_if_exists(manager: &SchemaManager<'_>, table: &str) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!("DROP TABLE IF EXISTS {table}"))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_accepts_only_its_own_database() {
        assert!(check_backend(BUILT_FOR).is_ok());
        let other = if BUILT_FOR == DatabaseBackend::MySql {
            DatabaseBackend::Postgres
        } else {
            DatabaseBackend::MySql
        };
        let message = check_backend(other).unwrap_err();
        assert!(message.contains("DATABASE_URL"), "{message}");
    }
}
