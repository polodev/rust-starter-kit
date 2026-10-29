use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Error, Result,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

pub struct AssignRole;

/// Minimal view of the `users` table, so this module does not depend on the app crate.
mod users {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub email: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub(super) async fn user_id_by_email(db: &sea_orm::DatabaseConnection, email: &str) -> Result<i64> {
    users::Entity::find()
        .filter(users::Column::Email.eq(email.trim().to_lowercase()))
        .one(db)
        .await?
        .map(|user| user.id)
        .ok_or_else(|| Error::string(&format!("no user with email {email}")))
}

#[async_trait]
impl Task for AssignRole {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "roles:assign".to_string(),
            detail: "Give a user a role (replaces the previous one).\nUsage: cargo run -- task roles:assign email:demo@gmail.com role:developer".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let email = vars
            .cli_arg("email")
            .map_err(|_| Error::string("email is mandatory"))?;
        let role = vars
            .cli_arg("role")
            .map_err(|_| Error::string("role is mandatory"))?;
        let user_id = user_id_by_email(&ctx.db, email).await?;
        let name = crate::assign_role(&ctx.db, user_id, role).await?;
        println!("{email} is now {name}");
        Ok(())
    }
}
