use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Result,
};

/// Deletes expired sessions. Scheduled hourly by the module.
pub struct PruneSessions;

#[async_trait]
impl Task for PruneSessions {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "auth:prune-sessions".to_string(),
            detail: "Delete expired login sessions.\nUsage: cargo run -- task auth:prune-sessions"
                .to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
        let removed = crate::session::prune(&ctx.db).await?;
        println!("removed {removed} expired session(s)");
        Ok(())
    }
}
