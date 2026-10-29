//! Demo account for local development.
//!
//! It has a public, known password, so it is created only in the development environment and
//! never in production or test.

use loco_rs::{app::AppContext, environment::Environment, Result};
use sea_orm::{DatabaseConnection, IntoActiveModel};

use crate::models::users::{self, RegisterParams};

pub const DEMO_NAME: &str = "Demo User";
pub const DEMO_EMAIL: &str = "demo@gmail.com";
pub const DEMO_PASSWORD: &str = "secret123$";

/// Creates the verified demo user with the `developer` role (every permission, including the
/// admin dashboard); running it again changes nothing except repairing a missing role.
pub async fn run(db: &DatabaseConnection) -> Result<()> {
    module_role_permission::sync(db).await?;

    let user = match users::Model::find_by_email(db, DEMO_EMAIL).await {
        Ok(user) => user,
        Err(_) => create(db).await?,
    };
    module_role_permission::assign_role(db, user.id, "developer").await?;
    Ok(())
}

async fn create(db: &DatabaseConnection) -> Result<users::Model> {
    let user = users::Model::create_with_password(
        db,
        &RegisterParams {
            email: DEMO_EMAIL.to_string(),
            password: DEMO_PASSWORD.to_string(),
            name: DEMO_NAME.to_string(),
        },
    )
    .await?;
    Ok(user.into_active_model().verified(db).await?)
}

/// Seeds the demo user only when running in development.
pub async fn run_in_development(ctx: &AppContext) -> Result<()> {
    if ctx.environment == Environment::Development {
        run(&ctx.db).await?;
    }
    Ok(())
}
