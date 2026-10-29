mod commands;
mod controllers;
mod locale;
mod migrations;
mod site;

use async_trait::async_trait;
use loco_modular::Module;
use loco_rs::{controller::Routes, task::Tasks};
use sea_orm_migration::MigrationTrait;

pub use locale::{fluent_tag, Locale, DEFAULT_LOCALE, SUPPORTED_LOCALES};
pub use site::{Site, SiteUser};

pub struct SiteLayoutModule;

#[async_trait]
impl Module for SiteLayoutModule {
    fn name(&self) -> &'static str {
        "site-layout"
    }

    fn routes(&self) -> Vec<Routes> {
        controllers::routes()
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
                "layouts/site.html",
                include_str!("../templates/layouts/site.html"),
            ),
            (
                "partials/language_switcher.html",
                include_str!("../templates/partials/language_switcher.html"),
            ),
            (
                "partials/theme_toggle.html",
                include_str!("../templates/partials/theme_toggle.html"),
            ),
            (
                "partials/user_menu.html",
                include_str!("../templates/partials/user_menu.html"),
            ),
            (
                "partials/language_switcher_compact.html",
                include_str!("../templates/partials/language_switcher_compact.html"),
            ),
            (
                "partials/guest_links.html",
                include_str!("../templates/partials/guest_links.html"),
            ),
            (
                "site/home.html",
                include_str!("../templates/site/home.html"),
            ),
            (
                "site/not_found.html",
                include_str!("../templates/site/not_found.html"),
            ),
        ]
    }
}
