//! Runs `mysqldump` (MySQL, MariaDB) or `pg_dump` (PostgreSQL), chosen by the address, and gzips the output into a
//! local file.

use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
};

use flate2::{write::GzEncoder, Compression};
use loco_rs::{Error, Result};

/// Which database server a backup is taken from (Spatie's `mysql` and `pgsql` dumpers).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Kind {
    Mysql,
    Postgres,
}

/// The parts of a `mysql://user:password@host:port/database?options` or `postgres://...` address.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Connection {
    pub kind: Kind,
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

/// Reads the app's database address (the same one the app connects with).
pub fn parse_database_uri(uri: &str) -> Result<Connection> {
    let (kind, rest) = if let Some(rest) = uri
        .strip_prefix("mysql://")
        .or_else(|| uri.strip_prefix("mariadb://"))
    {
        (Kind::Mysql, rest)
    } else if let Some(rest) = uri
        .strip_prefix("postgres://")
        .or_else(|| uri.strip_prefix("postgresql://"))
    {
        (Kind::Postgres, rest)
    } else {
        return Err(Error::string(
            "backups support MySQL, MariaDB (mysql://) and PostgreSQL (postgres://) databases only",
        ));
    };
    let (credentials, location) = rest.rsplit_once('@').unwrap_or(("", rest));
    let (username, password) = credentials.split_once(':').unwrap_or((credentials, ""));
    let (host_port, database) = location.split_once('/').unwrap_or((location, ""));
    let database = database.split('?').next().unwrap_or_default();
    let default_port = if kind == Kind::Postgres {
        "5432"
    } else {
        "3306"
    };
    let (host, port) = host_port
        .split_once(':')
        .unwrap_or((host_port, default_port));
    if host.is_empty() || database.is_empty() {
        return Err(Error::string(
            "the database address needs a host and a database name",
        ));
    }
    Ok(Connection {
        kind,
        host: host.to_string(),
        port: port.to_string(),
        database: percent_decode(database),
        username: if username.is_empty() {
            if kind == Kind::Postgres {
                "postgres"
            } else {
                "root"
            }
            .to_string()
        } else {
            percent_decode(username)
        },
        password: percent_decode(password),
    })
}

/// Which `mysqldump` this is. MySQL 8 clients ask a MariaDB server for a table it does not have unless told
/// `--column-statistics=0`; MariaDB's own client and MySQL 5.7 do not know that option.
#[derive(Debug, PartialEq, Eq)]
pub enum Flavour {
    Mysql8OrNewer,
    Other,
}

pub fn flavour_from_version(version_output: &str) -> Flavour {
    let lowered = version_output.to_lowercase();
    if lowered.contains("mariadb") {
        return Flavour::Other;
    }
    let major = lowered
        .split("ver ")
        .nth(1)
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|digits| digits.parse::<u32>().ok());
    match major {
        Some(major) if major >= 8 => Flavour::Mysql8OrNewer,
        _ => Flavour::Other,
    }
}

/// The dump program to run: the configured path, else `mysqldump` then `mariadb-dump` (MySQL, MariaDB), or
/// `pg_dump` (PostgreSQL; Homebrew keeps it outside the `PATH`, so its folders are tried too).
pub fn dump_binary(kind: Kind, configured: Option<&str>) -> Result<String> {
    let candidates: Vec<&str> = match (configured, kind) {
        (Some(path), _) if !path.trim().is_empty() => vec![path],
        (_, Kind::Mysql) => vec!["mysqldump", "mariadb-dump"],
        (_, Kind::Postgres) => vec![
            "pg_dump",
            "/usr/local/opt/postgresql@17/bin/pg_dump",
            "/usr/local/opt/postgresql@16/bin/pg_dump",
            "/opt/homebrew/opt/postgresql@17/bin/pg_dump",
            "/opt/homebrew/opt/postgresql@16/bin/pg_dump",
        ],
    };
    for candidate in &candidates {
        if Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|out| out.status.success())
        {
            return Ok((*candidate).to_string());
        }
    }
    let tools = match kind {
        Kind::Mysql => "the MySQL or MariaDB client tools",
        Kind::Postgres => "the PostgreSQL client tools (pg_dump)",
    };
    Err(Error::string(&format!(
        "could not run {}; install {tools}, or set settings.db_backup.dump_binary",
        candidates.join(" or ")
    )))
}

/// Arguments for the dump command (the password is passed in the environment, not here).
pub fn dump_args(connection: &Connection, flavour: &Flavour, extra: &[String]) -> Vec<String> {
    if connection.kind == Kind::Postgres {
        // `--no-owner` lets the dump be restored by any user; the password goes in PGPASSWORD.
        let mut args: Vec<String> = [
            "--host",
            &connection.host,
            "--port",
            &connection.port,
            "--username",
            &connection.username,
            "--no-password",
            "--no-owner",
        ]
        .iter()
        .map(|arg| (*arg).to_string())
        .collect();
        args.extend(extra.iter().cloned());
        args.push(connection.database.clone());
        return args;
    }
    let mut args: Vec<String> = [
        "--host",
        &connection.host,
        "--port",
        &connection.port,
        "--user",
        &connection.username,
        "--single-transaction",
        "--quick",
        "--skip-lock-tables",
        "--routines",
        "--triggers",
        "--default-character-set=utf8mb4",
    ]
    .iter()
    .map(|arg| (*arg).to_string())
    .collect();
    if *flavour == Flavour::Mysql8OrNewer {
        args.push("--column-statistics=0".to_string());
    }
    args.extend(extra.iter().cloned());
    args.push(connection.database.clone());
    args
}

/// Dumps the database into `target` as gzip. Returns the size of the compressed file.
/// Blocking: call it through `spawn_blocking`.
pub fn dump_to_gzip(
    binary: &str,
    connection: &Connection,
    extra: &[String],
    target: &Path,
) -> Result<u64> {
    let version = Command::new(binary)
        .arg("--version")
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default();
    let args = dump_args(connection, &flavour_from_version(&version), extra);

    let mut child = Command::new(binary)
        .args(&args)
        .env(
            if connection.kind == Kind::Postgres {
                "PGPASSWORD"
            } else {
                "MYSQL_PWD"
            },
            &connection.password,
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| Error::string(&format!("could not start {binary}: {error}")))?;
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    // Read the error text on its own thread so a chatty stderr cannot block the dump.
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });

    let file = File::create(target).map_err(|error| {
        Error::string(&format!("could not create {}: {error}", target.display()))
    })?;
    let mut encoder = GzEncoder::new(file, Compression::default());
    let mut buffer = vec![0u8; 256 * 1024];
    let mut dumped = 0u64;
    loop {
        let read = stdout
            .read(&mut buffer)
            .map_err(|error| Error::string(&error.to_string()))?;
        if read == 0 {
            break;
        }
        dumped += read as u64;
        encoder
            .write_all(&buffer[..read])
            .map_err(|error| Error::string(&error.to_string()))?;
    }
    let file = encoder
        .finish()
        .map_err(|error| Error::string(&error.to_string()))?;
    file.sync_all().ok();
    let status = child
        .wait()
        .map_err(|error| Error::string(&error.to_string()))?;
    let stderr_text = errors.join().unwrap_or_default();
    if !status.success() {
        let _ = std::fs::remove_file(target);
        return Err(Error::string(&format!(
            "{binary} failed ({status}): {}",
            stderr_text.trim().chars().take(600).collect::<String>()
        )));
    }
    if dumped == 0 {
        let _ = std::fs::remove_file(target);
        return Err(Error::string("the dump was empty"));
    }
    Ok(std::fs::metadata(target)
        .map(|meta| meta.len())
        .unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_database_address_is_split_and_decoded() {
        let full =
            parse_database_uri("mysql://app:p%40ss%3Aword@db.internal:3307/shop?ssl-mode=required")
                .unwrap();
        assert_eq!(
            (
                full.host.as_str(),
                full.port.as_str(),
                full.database.as_str(),
                full.username.as_str(),
                full.password.as_str()
            ),
            ("db.internal", "3307", "shop", "app", "p@ss:word")
        );
        let plain = parse_database_uri("mysql://root@127.0.0.1/rust_starter").unwrap();
        assert_eq!((plain.port.as_str(), plain.password.as_str()), ("3306", ""));
        for bad in [
            "sqlite://x.db",
            "mysql://root@host",
            "mysql:///db",
            "postgres://u@h",
            "",
        ] {
            assert!(parse_database_uri(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_client_flavour_decides_the_column_statistics_option() {
        assert_eq!(
            flavour_from_version("mysqldump  Ver 8.0.43 for macos14.7 on x86_64 (Homebrew)"),
            Flavour::Mysql8OrNewer
        );
        assert_eq!(
            flavour_from_version("mysqldump  Ver 10.19 Distrib 10.11.6-MariaDB, for Linux"),
            Flavour::Other
        );
        assert_eq!(
            flavour_from_version("mysqldump  Ver 5.7.44 for Linux"),
            Flavour::Other
        );
        assert_eq!(flavour_from_version(""), Flavour::Other);
    }

    #[test]
    fn arguments_never_carry_the_password() {
        let connection = parse_database_uri("mysql://app:secret@h:3306/shop").unwrap();
        let args = dump_args(
            &connection,
            &Flavour::Mysql8OrNewer,
            &["--no-tablespaces".to_string()],
        );
        assert!(!args.iter().any(|arg| arg.contains("secret")));
        assert_eq!(
            args.last().map(String::as_str),
            Some("shop"),
            "the database name comes last"
        );
        assert!(
            args.contains(&"--column-statistics=0".to_string())
                && args.contains(&"--no-tablespaces".to_string())
        );
        assert!(!dump_args(&connection, &Flavour::Other, &[])
            .contains(&"--column-statistics=0".to_string()));
    }

    #[test]
    fn a_postgres_address_is_understood_and_gets_pg_dump_arguments() {
        let pg = parse_database_uri("postgres://app:p%40ss@db.internal/shop").unwrap();
        assert_eq!(
            (
                pg.kind,
                pg.port.as_str(),
                pg.username.as_str(),
                pg.password.as_str()
            ),
            (Kind::Postgres, "5432", "app", "p@ss")
        );
        assert_eq!(
            parse_database_uri("postgresql://h:6543/d")
                .unwrap()
                .username,
            "postgres"
        );
        assert_eq!(parse_database_uri("mysql://h/d").unwrap().kind, Kind::Mysql);

        let args = dump_args(&pg, &Flavour::Other, &["--schema=public".to_string()]);
        assert!(
            !args.iter().any(|arg| arg.contains("p@ss")),
            "no password in arguments"
        );
        assert!(
            args.contains(&"--no-owner".to_string()) && args.contains(&"--no-password".to_string())
        );
        assert!(
            !args
                .iter()
                .any(|arg| arg.contains("single-transaction") || arg.contains("utf8mb4")),
            "no MySQL options"
        );
        assert_eq!(args.last().map(String::as_str), Some("shop"));
        assert_eq!(
            args[args.len() - 2],
            "--schema=public",
            "extra arguments before the name"
        );
    }
}
