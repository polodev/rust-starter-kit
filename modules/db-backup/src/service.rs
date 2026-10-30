//! `backup:run`, `backup:list`, `backup:clean` and `backup:monitor`, as plain functions so they can be tested
//! and called from other code.

use chrono::{DateTime, Duration, Utc};
use loco_rs::{Error, Result};

use crate::{
    config::BackupConfig,
    disks::{self, backup_path, valid_filename},
    dump::{self, Connection},
    retention::{self, Backup},
};

/// One file that was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub disk: String,
    pub path: String,
    pub size: u64,
}

fn chosen_disks(config: &BackupConfig, only: Option<&str>) -> Result<Vec<String>> {
    match only {
        Some(disk) if config.disks.contains_key(disk) => Ok(vec![disk.to_string()]),
        Some(disk) => Err(Error::string(&format!("`{disk}` is not a configured disk"))),
        None => Ok(config.destinations.clone()),
    }
}

/// Dumps the database once and stores the same file on every chosen disk. A disk that fails does not stop the
/// others; the error comes back at the end naming each failure.
pub async fn run(
    config: &BackupConfig,
    connection: &Connection,
    only_disk: Option<&str>,
    filename: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Vec<Stored>> {
    if let Some(name) = filename {
        if !valid_filename(name) {
            return Err(Error::string(
                "filename may only use letters, digits, - _ and .",
            ));
        }
    }
    let disks = chosen_disks(config, only_disk)?;
    let binary = dump::dump_binary(config.dump_binary.as_deref())?;

    let work = tempfile_path(now);
    let (binary_for_dump, connection_for_dump, extra, target) = (
        binary,
        connection.clone(),
        config.extra_dump_args.clone(),
        work.clone(),
    );
    let dumped = tokio::task::spawn_blocking(move || {
        dump::dump_to_gzip(&binary_for_dump, &connection_for_dump, &extra, &target)
    })
    .await
    .map_err(|error| Error::string(&error.to_string()))??;

    let remote = backup_path(&config.name, now, filename);
    let mut stored = Vec::new();
    let mut failures = Vec::new();
    for name in disks {
        let outcome = async {
            let op = disks::operator(&config.disks[&name])?;
            disks::upload(&op, &remote, &work).await
        }
        .await;
        match outcome {
            Ok(()) => stored.push(Stored {
                disk: name,
                path: remote.clone(),
                size: dumped,
            }),
            Err(error) => failures.push(format!("{name}: {error}")),
        }
    }
    let _ = tokio::fs::remove_file(&work).await;
    if failures.is_empty() {
        Ok(stored)
    } else {
        Err(Error::string(&format!(
            "backup failed on {}",
            failures.join("; ")
        )))
    }
}

fn tempfile_path(now: DateTime<Utc>) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "db-backup-{}-{}.sql.gz",
        now.format("%Y%m%d%H%M%S"),
        std::process::id()
    ))
}

/// Backups on one disk, oldest first.
pub async fn list(config: &BackupConfig, disk: &str) -> Result<Vec<Backup>> {
    let settings = config
        .disks
        .get(disk)
        .ok_or_else(|| Error::string(&format!("`{disk}` is not a configured disk")))?;
    disks::list(&disks::operator(settings)?, &config.name).await
}

/// What `backup:clean` removed (or would remove, with `dry_run`).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Cleaned {
    pub disk: String,
    pub deleted: Vec<String>,
}

/// Applies the retention strategy on every destination.
pub async fn clean(
    config: &BackupConfig,
    now: DateTime<Utc>,
    dry_run: bool,
) -> Result<Vec<Cleaned>> {
    let mut results = Vec::new();
    for name in &config.destinations {
        let backups = list(config, name).await?;
        let doomed = retention::to_delete(&backups, &config.retention, now);
        let mut cleaned = Cleaned {
            disk: name.clone(),
            deleted: Vec::new(),
        };
        let op = disks::operator(&config.disks[name])?;
        for index in doomed {
            if !dry_run {
                disks::delete(&op, &backups[index].path).await?;
            }
            cleaned.deleted.push(backups[index].path.clone());
        }
        results.push(cleaned);
    }
    Ok(results)
}

/// Health of one destination, like Spatie's `backup:monitor`.
#[derive(Debug, PartialEq, Eq)]
pub struct Health {
    pub disk: String,
    pub count: usize,
    pub newest: Option<DateTime<Utc>>,
    /// Why it is unhealthy, or `None` when it is fine.
    pub problem: Option<String>,
}

pub async fn monitor(config: &BackupConfig, now: DateTime<Utc>) -> Result<Vec<Health>> {
    let mut report = Vec::new();
    for name in &config.destinations {
        let backups = match list(config, name).await {
            Ok(backups) => backups,
            Err(error) => {
                report.push(Health {
                    disk: name.clone(),
                    count: 0,
                    newest: None,
                    problem: Some(format!("could not read the disk: {error}")),
                });
                continue;
            }
        };
        let newest = backups.last().map(|backup| backup.taken_at);
        let problem = match newest {
            None => Some("there are no backups".to_string()),
            Some(at) if now - at > Duration::days(config.monitor_max_age_days) => Some(format!(
                "the newest backup is {} day(s) old (allowed: {})",
                (now - at).num_days(),
                config.monitor_max_age_days
            )),
            Some(_) => None,
        };
        report.push(Health {
            disk: name.clone(),
            count: backups.len(),
            newest,
            problem,
        });
    }
    Ok(report)
}

/// Human-friendly size.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_readable() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }
}
