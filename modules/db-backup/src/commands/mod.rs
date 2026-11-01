use async_trait::async_trait;
use chrono::Utc;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Tasks, Vars},
    Error, Result,
};

use crate::{
    config::BackupConfig,
    dump::parse_database_uri,
    service::{self, human_size},
};

// mods-inject (do not remove)

fn flag(vars: &Vars, key: &str) -> bool {
    vars.cli_arg(key)
        .is_ok_and(|value| matches!(value, "true" | "1" | "yes"))
}

/// The configuration, or `None` after printing why a scheduled run was skipped. A run typed by hand
/// without configuration is an error.
fn config_or_skip(ctx: &AppContext, vars: &Vars) -> Result<Option<BackupConfig>> {
    match BackupConfig::load(ctx)? {
        Some(config) => Ok(Some(config)),
        None if flag(vars, "scheduled") => {
            println!("db-backup is not configured (settings.db_backup); skipping");
            Ok(None)
        }
        None => Err(Error::string(
            "db-backup is not configured: add settings.db_backup to config/<environment>.yaml (see modules/db-backup/README.md)",
        )),
    }
}

/// `backup:run`: like `php artisan backup:run --only-db`. Only the database is backed up, so `only-db` is
/// accepted for familiarity and changes nothing.
pub struct Run;

#[async_trait]
impl Task for Run {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "backup:run".to_string(),
            detail: "Dump the database (gzip) and store it on the configured backup disks.\nUsage: cargo run -- task backup:run only-db:true\n       cargo run -- task backup:run disk:r2 filename:before-upgrade".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let Some(config) = config_or_skip(ctx, vars)? else {
            return Ok(());
        };
        let connection = parse_database_uri(
            config
                .database_uri
                .as_deref()
                .unwrap_or(&ctx.config.database.uri),
        )?;
        println!("backing up database `{}` ...", connection.database);
        let stored = service::run(
            &config,
            &connection,
            vars.cli_arg("disk").ok(),
            vars.cli_arg("filename").ok(),
            Utc::now(),
        )
        .await?;
        for item in stored {
            println!(
                "stored on `{}`: {} ({})",
                item.disk,
                item.path,
                human_size(item.size)
            );
        }
        println!("backup finished");
        Ok(())
    }
}

/// `backup:list`.
pub struct List;

#[async_trait]
impl Task for List {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "backup:list".to_string(),
            detail: "List the backups on every destination.\nUsage: cargo run -- task backup:list"
                .to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let Some(config) = config_or_skip(ctx, vars)? else {
            return Ok(());
        };
        for disk in &config.destinations {
            let backups = service::list(&config, disk).await?;
            let total: u64 = backups.iter().map(|backup| backup.size).sum();
            println!("{disk}: {} backup(s), {}", backups.len(), human_size(total));
            for backup in &backups {
                println!(
                    "  {}  {}  {}",
                    backup.taken_at.format("%Y-%m-%d %H:%M:%S"),
                    human_size(backup.size),
                    backup.path
                );
            }
        }
        Ok(())
    }
}

/// `backup:clean`: applies the retention strategy.
pub struct Clean;

#[async_trait]
impl Task for Clean {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "backup:clean".to_string(),
            detail: "Delete old backups by the retention settings.\nUsage: cargo run -- task backup:clean\n       cargo run -- task backup:clean dry-run:true".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let Some(config) = config_or_skip(ctx, vars)? else {
            return Ok(());
        };
        let dry = flag(vars, "dry-run");
        for cleaned in service::clean(&config, Utc::now(), dry).await? {
            let verb = if dry { "would delete" } else { "deleted" };
            println!(
                "{}: {verb} {} old backup(s)",
                cleaned.disk,
                cleaned.deleted.len()
            );
            for path in cleaned.deleted {
                println!("  {path}");
            }
        }
        Ok(())
    }
}

/// `backup:monitor`: fails (non-zero exit) when a destination has no recent backup.
pub struct Monitor;

#[async_trait]
impl Task for Monitor {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "backup:monitor".to_string(),
            detail: "Check that every destination has a recent backup; exits with an error if not.\nUsage: cargo run -- task backup:monitor".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let Some(config) = config_or_skip(ctx, vars)? else {
            return Ok(());
        };
        let mut unhealthy = Vec::new();
        for health in service::monitor(&config, Utc::now()).await? {
            match &health.problem {
                None => println!(
                    "ok       {}: {} backup(s), newest {}",
                    health.disk,
                    health.count,
                    health
                        .newest
                        .map(|at| at.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_default()
                ),
                Some(problem) => {
                    println!("PROBLEM  {}: {problem}", health.disk);
                    unhealthy.push(health.disk.clone());
                }
            }
        }
        if unhealthy.is_empty() {
            Ok(())
        } else {
            Err(Error::string(&format!(
                "backup problem on {}",
                unhealthy.join(", ")
            )))
        }
    }
}

/// Commands of this module; run them with `cargo run -- task <signature>`.
pub fn register(tasks: &mut Tasks) {
    tasks.register(Run);
    tasks.register(List);
    tasks.register(Clean);
    tasks.register(Monitor);
    // tasks-inject (do not remove)
}
