mod commands;
mod controllers;
mod entities;
mod migrations;
mod seeds;

use async_trait::async_trait;
use loco_modular::{MenuItem, Module, ScheduledJob};
use loco_rs::{app::AppContext, controller::Routes, task::Tasks, Result};
use module_media::{CollectionDef, HasMedia, DEFAULT_MAX_BYTES, IMAGES};
use sea_orm_migration::MigrationTrait;

pub use entities::{page_translations, pages};

/// Name of the single-file media collection that holds a page's feature image.
pub const FEATURE_IMAGE: &str = "feature_image";

/// The feature image is a single-file collection: a new upload replaces the old one. It is
/// public because it is shown on the public page.
pub const fn feature_image_collection() -> CollectionDef {
    CollectionDef::single(FEATURE_IMAGE)
        .accepts_mime_types(IMAGES)
        .max_bytes(DEFAULT_MAX_BYTES)
        .public()
}

/// A page owns files through the media library, like a Spatie `HasMedia` model.
impl HasMedia for pages::Model {
    fn media_type(&self) -> String {
        "page".to_string()
    }

    fn media_id(&self) -> i64 {
        self.id
    }

    fn media_collections(&self) -> Vec<CollectionDef> {
        vec![feature_image_collection()]
    }
}

pub struct PagesModule;

#[async_trait]
impl Module for PagesModule {
    fn name(&self) -> &'static str {
        "pages"
    }

    fn routes(&self) -> Vec<Routes> {
        controllers::routes()
    }

    fn schedule(&self) -> Vec<ScheduledJob> {
        vec![ScheduledJob::command("pages:check-translations")
            .daily_at(3, 0)
            .tag("content")]
    }

    fn register_tasks(&self, tasks: &mut Tasks) {
        commands::register(tasks);
    }

    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::all()
    }

    fn templates(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "pages/show.html",
                include_str!("../templates/pages/show.html"),
            ),
            (
                "admin/pages.html",
                include_str!("../templates/admin/pages.html"),
            ),
            (
                "admin/page_edit.html",
                include_str!("../templates/admin/page_edit.html"),
            ),
        ]
    }

    fn admin_menu(&self) -> Vec<MenuItem> {
        vec![MenuItem {
            section: "Content",
            label: "Pages",
            href: "/admin/pages",
            permission: "pages.view",
            icon: "fa-regular fa-file-lines",
            order: 1,
            match_children: true,
        }]
    }

    async fn seed(&self, ctx: &AppContext) -> Result<()> {
        seeds::run(&ctx.db).await
    }
}
