use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Result,
};

pub struct SyncRoles;

#[async_trait]
impl Task for SyncRoles {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "roles:sync".to_string(),
            detail: "Make the database match the roles and permissions defined in code.\nUsage: cargo run -- task roles:sync".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
        let report = crate::sync(&ctx.db).await?;
        println!(
            "permissions: {} created, {} updated, {} deactivated",
            report.permissions_created, report.permissions_updated, report.permissions_deactivated
        );
        println!(
            "roles:       {} created, {} updated, {} deactivated",
            report.roles_created, report.roles_updated, report.roles_deactivated
        );
        Ok(())
    }
}
