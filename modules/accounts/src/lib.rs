mod commands;
mod controllers;
mod logo;
mod migrations;

use async_trait::async_trait;
use axum::middleware::from_fn_with_state;
use loco_modular::Module;
use loco_rs::{app::AppContext, controller::Routes, task::Tasks};
use sea_orm_migration::MigrationTrait;

pub use logo::{logo_collection, user_owner, LOGO_COLLECTION};

/// The signed-in person's own area: profile, password and logo, inside one sub-layout.
pub struct AccountsModule;

#[async_trait]
impl Module for AccountsModule {
    fn name(&self) -> &'static str {
        "accounts"
    }

    fn routes(&self) -> Vec<Routes> {
        controllers::routes()
    }

    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::all()
    }

    fn register_tasks(&self, tasks: &mut Tasks) {
        commands::register(tasks);
    }

    fn templates(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "accounts/layout.html",
                include_str!("../templates/accounts/layout.html"),
            ),
            (
                "accounts/profile.html",
                include_str!("../templates/accounts/profile.html"),
            ),
            (
                "accounts/password.html",
                include_str!("../templates/accounts/password.html"),
            ),
            (
                "accounts/logo.html",
                include_str!("../templates/accounts/logo.html"),
            ),
        ]
    }

    /// Runs after the session is known (the auth layer is applied later, so it wraps this one) and
    /// puts the person's logo into the header avatar.
    fn apply(&self, router: axum::Router, ctx: &AppContext) -> axum::Router {
        router.layer(from_fn_with_state(ctx.clone(), logo::attach_avatar))
    }
}
