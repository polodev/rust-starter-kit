mod commands;
mod controllers;
mod entities;
mod migrations;
mod seeds;

use async_trait::async_trait;
use loco_modular::{Module, ScheduledJob};
use loco_rs::{app::AppContext, controller::Routes, task::Tasks, Result};
use sea_orm_migration::MigrationTrait;

pub use entities::{page_translations, pages};

pub struct PagesModule;

#[async_trait]
impl Module for PagesModule {
    fn name(&self) -> &'static str {
        "pages"
    }

    fn routes(&self) -> Vec<Routes> {
        controllers::routes()
    }

    fn register_tasks(&self, tasks: &mut Tasks) {
        commands::register(tasks);
    }

    fn schedule(&self) -> Vec<ScheduledJob> {
        vec![ScheduledJob::command("pages:check-translations")
            .daily_at(3, 0)
            .tag("content")]
    }

    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::all()
    }

    fn templates(&self) -> Vec<(&'static str, &'static str)> {
        vec![(
            "pages/show.html",
            include_str!("../templates/pages/show.html"),
        )]
    }

    async fn seed(&self, ctx: &AppContext) -> Result<()> {
        seeds::run(&ctx.db).await
    }
}
