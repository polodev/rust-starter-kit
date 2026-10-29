use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Result,
};

/// Deletes expired API tokens. Scheduled daily by the module.
pub struct PruneTokens;

#[async_trait]
impl Task for PruneTokens {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "auth-api:prune-tokens".to_string(),
            detail: "Delete expired API tokens.\nUsage: cargo run -- task auth-api:prune-tokens"
                .to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
        let removed = crate::api_token::prune(&ctx.db).await?;
        println!("removed {removed} expired api token(s)");
        Ok(())
    }
}
