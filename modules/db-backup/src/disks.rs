//! Talking to a backup disk (a local folder or an S3-compatible bucket) through OpenDAL.

use std::path::Path;

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use loco_rs::{Error, Result};
use opendal::{services, Operator};
use tokio::io::AsyncReadExt;

use crate::{config::DiskConfig, retention::Backup};

const TIMESTAMP_FORMAT: &str = "%Y-%m-%d-%H-%M-%S";
const SUFFIX: &str = ".sql.gz";
const UPLOAD_CHUNK: usize = 8 * 1024 * 1024;

fn failure(error: opendal::Error) -> Error {
    Error::string(&error.to_string())
}

/// OpenDAL 0.58 has no HTTP client until one is installed; do it once, before the first S3 disk is opened.
fn install_http_client() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(opendal::install_default);
}

/// Opens a disk.
pub fn operator(disk: &DiskConfig) -> Result<Operator> {
    match disk {
        DiskConfig::Local { path } => {
            Operator::new(services::Fs::default().root(path)).map_err(failure)
        }
        DiskConfig::S3 {
            bucket,
            region,
            endpoint,
            access_key_id,
            secret_access_key,
            prefix,
        } => {
            install_http_client();
            let mut builder = services::S3::default()
                .bucket(bucket)
                .region(region)
                .access_key_id(access_key_id)
                .secret_access_key(secret_access_key);
            if let Some(endpoint) = endpoint
                .as_deref()
                .filter(|endpoint| !endpoint.trim().is_empty())
            {
                builder = builder.endpoint(endpoint);
            }
            if let Some(prefix) = prefix
                .as_deref()
                .map(|p| p.trim_matches('/'))
                .filter(|p| !p.is_empty())
            {
                builder = builder.root(&format!("/{prefix}"));
            }
            Operator::new(builder).map_err(failure)
        }
    }
}

/// `{name}/{YYYY-MM-DD-HH-MM-SS}.sql.gz` (or the given file name).
pub fn backup_path(name: &str, taken_at: DateTime<Utc>, filename: Option<&str>) -> String {
    match filename {
        Some(filename) => format!("{name}/{filename}{SUFFIX}"),
        None => format!("{name}/{}{SUFFIX}", taken_at.format(TIMESTAMP_FORMAT)),
    }
}

/// The time a backup was taken, read from its file name.
pub fn taken_at_from(path: &str) -> Option<DateTime<Utc>> {
    let file = path.rsplit('/').next()?.strip_suffix(SUFFIX)?;
    let naive = NaiveDateTime::parse_from_str(file, TIMESTAMP_FORMAT).ok()?;
    Some(Utc.from_utc_datetime(&naive))
}

/// A file name for `filename:` that cannot escape the folder.
pub fn valid_filename(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// Streams a local file to the disk, in chunks, so a large dump never sits in memory.
pub async fn upload(op: &Operator, remote: &str, local: &Path) -> Result<()> {
    let mut writer = op.writer(remote).await.map_err(failure)?;
    let mut file = tokio::fs::File::open(local)
        .await
        .map_err(|error| Error::string(&error.to_string()))?;
    let mut buffer = vec![0u8; UPLOAD_CHUNK];
    loop {
        let mut filled = 0;
        while filled < buffer.len() {
            let read = file
                .read(&mut buffer[filled..])
                .await
                .map_err(|error| Error::string(&error.to_string()))?;
            if read == 0 {
                break;
            }
            filled += read;
        }
        if filled == 0 {
            break;
        }
        writer
            .write(buffer[..filled].to_vec())
            .await
            .map_err(failure)?;
        if filled < buffer.len() {
            break;
        }
    }
    writer.close().await.map_err(failure)?;
    Ok(())
}

/// The backups in `{name}/`, oldest first. Files whose name is not a backup timestamp are ignored.
pub async fn list(op: &Operator, name: &str) -> Result<Vec<Backup>> {
    let entries = match op.list(&format!("{name}/")).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == opendal::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(failure(error)),
    };
    let mut backups = Vec::new();
    for entry in entries {
        let path = entry.path().to_string();
        let Some(taken_at) = taken_at_from(&path) else {
            continue;
        };
        let size = match entry.metadata().content_length() {
            0 => op
                .stat(&path)
                .await
                .map(|meta| meta.content_length())
                .unwrap_or(0),
            known => known,
        };
        backups.push(Backup {
            path,
            size,
            taken_at,
        });
    }
    backups.sort_by_key(|backup| backup.taken_at);
    Ok(backups)
}

pub async fn delete(op: &Operator, path: &str) -> Result<()> {
    op.delete(path).await.map_err(failure)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disks_can_be_opened_without_touching_the_network() {
        // Building the clients checks the settings; nothing is sent until a backup is stored.
        let r2 = DiskConfig::S3 {
            bucket: "backups".into(),
            region: "auto".into(),
            endpoint: Some("https://account.r2.cloudflarestorage.com".into()),
            access_key_id: "key".into(),
            secret_access_key: "secret".into(),
            prefix: Some("/db/".into()),
        };
        assert!(operator(&r2).is_ok());
        assert!(operator(&DiskConfig::Local {
            path: "storage/backups".into()
        })
        .is_ok());
    }

    #[test]
    fn names_round_trip_and_stay_inside_the_folder() {
        let at = Utc.with_ymd_and_hms(2026, 10, 30, 1, 30, 5).unwrap();
        let path = backup_path("my-app", at, None);
        assert_eq!(path, "my-app/2026-10-30-01-30-05.sql.gz");
        assert_eq!(taken_at_from(&path), Some(at));
        assert_eq!(
            backup_path("my-app", at, Some("before-upgrade")),
            "my-app/before-upgrade.sql.gz"
        );
        assert_eq!(taken_at_from("my-app/before-upgrade.sql.gz"), None);
        assert_eq!(taken_at_from("my-app/2026-10-30-01-30-05.zip"), None);
        for good in ["before-upgrade", "v1.2", "a_b"] {
            assert!(valid_filename(good), "{good}");
        }
        for bad in ["", ".hidden", "../x", "a/b", "a b", &"x".repeat(101)] {
            assert!(!valid_filename(bad), "{bad}");
        }
    }
}
