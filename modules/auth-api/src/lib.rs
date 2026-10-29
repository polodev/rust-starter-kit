//! A Laravel-style token API (think Sanctum): register, login, user, logout and token management
//! under `/api/v1/auth`. Other API modules protect their routes by taking `ApiAuth` as an argument.

mod api_token;
mod commands;
mod controllers;
mod entities;
mod guard;
mod json;
mod migrations;

use async_trait::async_trait;
use loco_modular::{Module, ScheduledJob};
use loco_rs::{controller::Routes, task::Tasks};
use sea_orm_migration::MigrationTrait;

pub use api_token::{create as create_api_token, revoke_others as revoke_api_tokens};
pub use entities::personal_access_tokens;
pub use guard::{ApiAuth, ApiUser};
pub use json::{message as json_message, unauthenticated, Errors as ValidationErrors};

pub struct AuthApiModule;

#[async_trait]
impl Module for AuthApiModule {
    fn name(&self) -> &'static str {
        "auth-api"
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

    fn schedule(&self) -> Vec<ScheduledJob> {
        vec![ScheduledJob::command("auth-api:prune-tokens")
            .daily_at(3, 30)
            .tag("maintenance")]
    }
}
