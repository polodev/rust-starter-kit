use std::{path::PathBuf, process::Command};

use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    environment::Environment,
    task::{Task, TaskInfo, Vars},
    Error, Result,
};

/// Where the PHP side lives: next to this crate's sources, so it works from any directory.
fn php_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("php")
}

/// The pieces of a `mysql://user:password@host:port/database?options` address.
#[derive(Debug, PartialEq, Eq)]
pub struct Connection {
    pub host: String,
    pub port: String,
    pub database: String,
    pub username: String,
    pub password: String,
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(&text[index + 1..index + 3], 16) {
                out.push(value);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Reads the app's own database address, so the PHP side needs no configuration of its own.
pub fn parse_database_uri(uri: &str) -> Result<Connection> {
    let rest = uri
        .strip_prefix("mysql://")
        .or_else(|| uri.strip_prefix("mariadb://"))
        .ok_or_else(|| Error::string("only mysql:// database addresses are supported"))?;
    let (credentials, location) = rest.rsplit_once('@').unwrap_or(("", rest));
    let (username, password) = credentials.split_once(':').unwrap_or((credentials, ""));
    let (host_port, database) = location.split_once('/').unwrap_or((location, ""));
    let database = database.split('?').next().unwrap_or_default();
    let (host, port) = host_port.split_once(':').unwrap_or((host_port, "3306"));
    if host.is_empty() || database.is_empty() {
        return Err(Error::string(
            "the database address needs a host and a database name",
        ));
    }
    Ok(Connection {
        host: host.to_string(),
        port: port.to_string(),
        database: percent_decode(database),
        username: if username.is_empty() {
            "root".to_string()
        } else {
            percent_decode(username)
        },
        password: percent_decode(password),
    })
}

/// `cargo run -- task tinker:eloquent`: opens PsySH (the engine of `php artisan tinker`) with
/// Eloquent models and the `DB` facade connected to this app's database.
pub struct TinkerEloquent;

#[async_trait]
impl Task for TinkerEloquent {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "tinker:eloquent".to_string(),
            detail: "Laravel-style tinker (Eloquent models and the DB facade) on this app's database.\nUsage: cargo run -- task tinker:eloquent\n       cargo run -- task tinker:eloquent run:\"User::count()\"   (one expression)\nNeeds PHP 8.2+ and Composer; dependencies install on first use.\nIn production add confirm:true.".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        if ctx.environment == Environment::Production
            && !vars.cli_arg("confirm").is_ok_and(|value| value == "true")
        {
            return Err(Error::string(
                "this console can change data; in production add confirm:true",
            ));
        }
        let connection = parse_database_uri(&ctx.config.database.uri)?;
        let dir = php_dir();

        if !dir.join("vendor/autoload.php").exists() {
            println!("installing the PHP packages (first run only)...");
            let status = Command::new("composer")
                .args(["install", "--no-interaction", "--prefer-dist"])
                .current_dir(&dir)
                .status()
                .map_err(|error| {
                    Error::string(&format!(
                        "could not run composer: {error}. Install PHP 8.2+ and Composer."
                    ))
                })?;
            if !status.success() {
                return Err(Error::string("composer install failed"));
            }
        }

        let mut command = Command::new("php");
        command
            .arg("tinker.php")
            .current_dir(&dir)
            .env("DB_HOST", connection.host)
            .env("DB_PORT", connection.port)
            .env("DB_DATABASE", connection.database)
            .env("DB_USERNAME", connection.username)
            .env("DB_PASSWORD", connection.password);
        if let Ok(code) = vars.cli_arg("run") {
            command.env("TINKER_EXEC", code);
        }
        // The shell is interactive and blocks until the person leaves it.
        let status = tokio::task::block_in_place(|| command.status()).map_err(|error| {
            Error::string(&format!("could not run php: {error}. Install PHP 8.2+."))
        })?;
        if status.success() {
            Ok(())
        } else {
            Err(Error::string("the PHP console exited with an error"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_database_address_is_split_into_parts() {
        assert_eq!(
            parse_database_uri("mysql://root@127.0.0.1:3306/rust_starter").unwrap(),
            Connection {
                host: "127.0.0.1".into(),
                port: "3306".into(),
                database: "rust_starter".into(),
                username: "root".into(),
                password: String::new(),
            }
        );
        let full =
            parse_database_uri("mysql://app:p%40ss%3Aword@db.internal/shop?ssl-mode=required")
                .unwrap();
        assert_eq!(
            (
                full.host.as_str(),
                full.port.as_str(),
                full.database.as_str(),
                full.username.as_str(),
                full.password.as_str()
            ),
            ("db.internal", "3306", "shop", "app", "p@ss:word")
        );
    }

    #[test]
    fn unsupported_or_incomplete_addresses_are_refused() {
        for bad in ["postgres://u@h/d", "mysql://root@host", "mysql:///db", ""] {
            assert!(parse_database_uri(bad).is_err(), "{bad}");
        }
    }
}
