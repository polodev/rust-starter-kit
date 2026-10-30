use std::io::Read;

use chrono::{Duration, Utc};
use loco_rs::testing::prelude::*;
use module_db_backup::{
    clean, list, monitor, parse_database_uri, run, BackupConfig, DiskConfig, Retention,
};
use serde_json::json;
use serial_test::serial;

use rust_starter::app::App;

fn dump_tool_missing() -> bool {
    std::process::Command::new("mysqldump")
        .arg("--version")
        .output()
        .map_or(true, |out| !out.status.success())
        && std::process::Command::new("mariadb-dump")
            .arg("--version")
            .output()
            .map_or(true, |out| !out.status.success())
}

fn config(folder: &std::path::Path) -> BackupConfig {
    BackupConfig::from_value(json!({
        "name": "test-app",
        "destinations": ["first", "second"],
        "disks": {
            "first": { "driver": "local", "path": folder.join("first").to_str().unwrap() },
            "second": { "driver": "local", "path": folder.join("second").to_str().unwrap() }
        },
        "retention": { "keep_all_days": 2, "keep_daily_days": 3, "keep_weekly_weeks": 0, "keep_monthly_months": 0, "keep_yearly_years": 0 },
        "monitor_max_age_days": 1
    }))
    .unwrap()
}

#[tokio::test]
#[serial]
async fn a_backup_is_a_gzipped_dump_stored_on_every_disk_and_then_listed_cleaned_and_monitored() {
    if dump_tool_missing() {
        eprintln!("skipped: mysqldump is not installed");
        return;
    }
    let ctx = boot_test::<App>().await.unwrap().app_context;
    let connection = parse_database_uri(&ctx.config.database.uri).unwrap();
    let folder = std::env::temp_dir().join(format!("db-backup-test-{}", std::process::id()));
    let config = config(&folder);

    // Nothing yet: the monitor complains, the list is empty.
    let before = monitor(&config, Utc::now()).await.unwrap();
    assert!(before
        .iter()
        .all(|health| health.problem.as_deref() == Some("there are no backups")));
    assert!(list(&config, "first").await.unwrap().is_empty());

    // `backup:run`: one dump, written to both disks.
    let now = Utc::now();
    let stored = run(&config, &connection, None, None, now).await.unwrap();
    assert_eq!(stored.len(), 2);
    let expected = format!("test-app/{}.sql.gz", now.format("%Y-%m-%d-%H-%M-%S"));
    assert!(stored
        .iter()
        .all(|item| item.path == expected && item.size > 100));
    for disk in ["first", "second"] {
        let on_disk = folder.join(disk).join(&expected);
        let mut text = String::new();
        flate2_read(&on_disk, &mut text);
        assert!(
            text.contains("CREATE TABLE `users`") && text.contains("CREATE TABLE `media`"),
            "{disk}: a real dump"
        );
        assert!(!text.contains(&connection.password) || connection.password.is_empty());
    }

    // Only one disk, with a chosen name.
    let named = run(
        &config,
        &connection,
        Some("second"),
        Some("before-upgrade"),
        now,
    )
    .await
    .unwrap();
    assert_eq!(named.len(), 1);
    assert!(
        folder
            .join("second/test-app/before-upgrade.sql.gz")
            .exists()
            && !folder.join("first/test-app/before-upgrade.sql.gz").exists()
    );
    assert!(run(&config, &connection, Some("nowhere"), None, now)
        .await
        .is_err());
    assert!(run(&config, &connection, None, Some("../escape"), now)
        .await
        .is_err());

    // The list sees timestamped backups only (the named one is not part of the schedule).
    let first = list(&config, "first").await.unwrap();
    assert_eq!(first.len(), 1);

    // Old backups: put copies under older timestamps, then clean. Retention keeps 2 days of everything,
    // then one per day for 3 more days; nothing beyond that.
    let source = folder.join("first").join(&expected);
    for days in [1, 3, 4, 9] {
        let name = (now - Duration::days(days))
            .format("%Y-%m-%d-%H-%M-%S")
            .to_string();
        std::fs::copy(
            &source,
            folder.join("first/test-app").join(format!("{name}.sql.gz")),
        )
        .unwrap();
    }
    assert_eq!(list(&config, "first").await.unwrap().len(), 5);
    let preview = clean(&config, now, true).await.unwrap();
    assert_eq!(
        preview[0].deleted.len(),
        1,
        "only the 9-day-old backup is past the strategy"
    );
    assert_eq!(
        list(&config, "first").await.unwrap().len(),
        5,
        "a dry run deletes nothing"
    );
    let cleaned = clean(&config, now, false).await.unwrap();
    assert_eq!(cleaned[0].deleted.len(), 1);
    assert_eq!(list(&config, "first").await.unwrap().len(), 4);

    // The monitor is happy with a fresh backup and unhappy with a stale one.
    assert!(monitor(&config, now)
        .await
        .unwrap()
        .iter()
        .all(|health| health.problem.is_none()));
    let later = monitor(&config, now + Duration::days(3)).await.unwrap();
    assert!(later[0].problem.as_deref().unwrap().contains("day(s) old"));

    let _ = std::fs::remove_dir_all(&folder);
}

fn flate2_read(path: &std::path::Path, out: &mut String) {
    let file = std::fs::File::open(path).unwrap();
    let mut decoder = flate2::read::GzDecoder::new(file);
    decoder.read_to_string(out).unwrap();
}

#[tokio::test]
#[serial]
async fn a_failing_dump_tool_or_bad_database_is_reported_and_leaves_nothing_behind() {
    if dump_tool_missing() {
        return;
    }
    let folder = std::env::temp_dir().join(format!("db-backup-fail-{}", std::process::id()));
    let config = config(&folder);
    let mut connection =
        parse_database_uri("mysql://root@127.0.0.1:3306/rust_starter_test").unwrap();
    connection.database = "no_such_database_here".to_string();
    let error = run(&config, &connection, None, None, Utc::now())
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("failed") || error.contains("Unknown database"),
        "{error}"
    );
    assert!(
        !folder.join("first/test-app").exists()
            || std::fs::read_dir(folder.join("first/test-app"))
                .unwrap()
                .next()
                .is_none()
    );
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn unconfigured_s3_credentials_and_the_default_retention_match_spatie() {
    assert_eq!(
        Retention::default(),
        Retention {
            keep_all_days: 7,
            keep_daily_days: 16,
            keep_weekly_weeks: 8,
            keep_monthly_months: 4,
            keep_yearly_years: 2
        }
    );
    let broken = BackupConfig::from_value(json!({
        "destinations": ["r2"],
        "disks": { "r2": { "driver": "s3", "bucket": "b", "access_key_id": "", "secret_access_key": "" } }
    }));
    assert!(broken.unwrap_err().to_string().contains("needs a bucket"));
    let _ = DiskConfig::Local { path: "x".into() };
}
