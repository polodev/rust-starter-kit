use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Error, Result,
};

pub struct RevokeRole;

#[async_trait]
impl Task for RevokeRole {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "roles:revoke".to_string(),
            detail:
                "Remove a user's role.\nUsage: cargo run -- task roles:revoke email:demo@gmail.com"
                    .to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let email = vars
            .cli_arg("email")
            .map_err(|_| Error::string("email is mandatory"))?;
        let user_id = super::assign_role::user_id_by_email(&ctx.db, email).await?;
        if crate::revoke_role(&ctx.db, user_id).await? {
            println!("{email} no longer has a role");
        } else {
            println!("{email} had no role");
        }
        Ok(())
    }
}
