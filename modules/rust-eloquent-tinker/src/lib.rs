//! A development console in the style of `php artisan tinker`: Eloquent models and the `DB` facade
//! on this app's database. The PHP side is in `php/`; this crate only adds the command that starts it.

mod commands;
mod controllers;
mod migrations;

use async_trait::async_trait;
use loco_modular::Module;
use loco_rs::{controller::Routes, task::Tasks};
use sea_orm_migration::MigrationTrait;

pub struct RustEloquentTinkerModule;

#[async_trait]
impl Module for RustEloquentTinkerModule {
    fn name(&self) -> &'static str {
        "rust-eloquent-tinker"
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
