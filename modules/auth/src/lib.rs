mod commands;
mod controllers;
mod entities;
mod guard;
mod middleware;
mod migrations;
pub mod session;

use async_trait::async_trait;
use axum::middleware::from_fn_with_state;
use loco_modular::{Module, ScheduledJob};
use loco_rs::{app::AppContext, controller::Routes, task::Tasks};
use sea_orm_migration::MigrationTrait;

pub use entities::{sessions, users};
pub use guard::{safe_next, same_origin, Auth, AuthUser, OptionalAuth};

pub struct AuthModule;

#[async_trait]
impl Module for AuthModule {
    fn name(&self) -> &'static str {
        "auth"
    }

    fn routes(&self) -> Vec<Routes> {
        controllers::routes()
    }

    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::all()
    }

    fn templates(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "auth/login.html",
                include_str!("../templates/auth/login.html"),
            ),
            (
                "auth/register.html",
                include_str!("../templates/auth/register.html"),
            ),
        ]
    }

    fn register_tasks(&self, tasks: &mut Tasks) {
        commands::register(tasks);
    }

    fn schedule(&self) -> Vec<ScheduledJob> {
        vec![ScheduledJob::command("auth:prune-sessions")
            .hourly()
            .tag("maintenance")]
    }

    fn apply(&self, router: axum::Router, ctx: &AppContext) -> axum::Router {
        router.layer(from_fn_with_state(ctx.clone(), middleware::load_session))
    }
}
