use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Error, Result,
};

/// `activitylog:clean`, like Spatie's command. Scheduled daily by the module.
pub struct Clean;

#[async_trait]
impl Task for Clean {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "activitylog:clean".to_string(),
            detail: "Delete old activity log entries (default: older than 365 days).\nUsage: cargo run -- task activitylog:clean days:90 log:users".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let days: i64 = match vars.cli_arg("days") {
            Ok(text) => text
                .parse()
                .map_err(|_| Error::string("days must be a whole number"))?,
            Err(_) => crate::DEFAULT_RETENTION_DAYS,
        };
        let log = vars.cli_arg("log").ok();
        let removed = loco_activity_log::clean(&ctx.db, days, log).await?;
        println!("deleted {removed} record(s) from the activity log");
        Ok(())
    }
}
