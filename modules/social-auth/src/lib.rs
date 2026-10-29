//! Sign in with Google for the website and the API, linked to local users through the
//! `social_accounts` table.

mod commands;
mod controllers;
mod entities;
pub mod google;
mod migrations;
mod service;

use async_trait::async_trait;
use loco_modular::Module;
use loco_rs::{controller::Routes, task::Tasks};
use sea_orm_migration::MigrationTrait;

pub use entities::social_accounts;
pub use service::find_or_create_user;

pub struct SocialAuthModule;

#[async_trait]
impl Module for SocialAuthModule {
    fn name(&self) -> &'static str {
        "social-auth"
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
}
