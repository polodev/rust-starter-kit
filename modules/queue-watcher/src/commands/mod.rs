use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Tasks, Vars},
    Error, Result,
};

// mods-inject (do not remove)

fn whole_number(vars: &Vars, key: &str, default: i64) -> Result<i64> {
    match vars.cli_arg(key) {
        Ok(text) => text
            .parse::<i64>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| Error::string(&format!("{key} must be a positive whole number"))),
        Err(_) => Ok(default),
    }
}

/// `queue-watcher:purge`: deletes old finished, failed and stale rows. Scheduled daily.
pub struct Purge;

#[async_trait]
impl Task for Purge {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "queue-watcher:purge".to_string(),
            detail: "Delete finished, failed and stale job history older than N days (default 14).\nUsage: cargo run -- task queue-watcher:purge days:7".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let days = whole_number(vars, "days", crate::RETENTION_DAYS)?;
        let removed = crate::tracker::purge(&ctx.db, days).await?;
        println!("removed {removed} old job record(s)");
        Ok(())
    }
}

/// `queue-watcher:mark-stale`: flags jobs that have been running for too long. Scheduled hourly.
pub struct MarkStale;

#[async_trait]
impl Task for MarkStale {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "queue-watcher:mark-stale".to_string(),
            detail: "Mark jobs that have run for more than N hours (default 1) as stale.\nUsage: cargo run -- task queue-watcher:mark-stale hours:2".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let hours = whole_number(vars, "hours", crate::STALE_AFTER_HOURS)?;
        let marked = crate::tracker::mark_stale(&ctx.db, hours).await?;
        println!("marked {marked} job(s) as stale");
        Ok(())
    }
}

/// Commands of this module; run them with `cargo run -- task <signature>`.
pub fn register(tasks: &mut Tasks) {
    tasks.register(Purge);
    tasks.register(MarkStale);
    // `jobs:work` needs the handlers of every module, so the app registers it (src/app.rs).
    tasks.register(crate::JobsFailed);
    tasks.register(crate::JobsRetryFailed);
    tasks.register(crate::JobsFlushFailed);
    // tasks-inject (do not remove)
}
