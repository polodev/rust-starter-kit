//! Media library, modelled on Spatie's laravel-medialibrary. It uses Spatie's own `media` table and
//! its `{id}/{file_name}` file layout, so a Laravel app and this app can share one database and one
//! bucket, and existing Laravel media can be imported with `media:import`.
//!
//! Any model can own files by implementing [`HasMedia`]. Files live in named *collections*, and a
//! collection is either single-file (adding a file replaces the old one, like a logo) or multiple
//! (many files, optionally keeping only the newest N, like a gallery).
//!
//! ```ignore
//! let owner = Owner::new("user", user.id).with_collections(vec![
//!     CollectionDef::single("logo").accepts_mime_types(IMAGES).max_bytes(1 << 20),
//!     CollectionDef::multiple("gallery").only_keep_latest(10).accepts_mime_types(IMAGES),
//! ]);
//! owner.add_media(&ctx, Upload::new("logo.png", bytes)).using_name("Company logo").to_media_collection("logo").await?;
//! let url = owner.get_first_media_url(&ctx, "logo").await?;
//! ```

mod collection;
mod commands;
mod controllers;
mod entities;
mod error;
mod has_media;
mod import;
mod migrations;
mod service;
mod upload;

use async_trait::async_trait;
use loco_modular::{MenuItem, Module};
use loco_rs::{controller::Routes, task::Tasks};
use sea_orm_migration::MigrationTrait;

pub use collection::{CollectionDef, CollectionKind, DEFAULT_MAX_BYTES, DOCUMENTS, IMAGES};
pub use controllers::{read_form, read_upload, ParsedForm};
pub use entities::media;
pub use error::{MediaError, MediaResult};
pub use has_media::{HasMedia, InteractsWithMedia, Owner};
pub use service::{url_for, FileAdder};
pub use upload::Upload;

/// Loose files uploaded straight into the media library belong to this built-in owner.
pub const LIBRARY_OWNER_TYPE: &str = "library";
pub const LIBRARY_OWNER_ID: i64 = 0;
pub const LIBRARY_COLLECTION: &str = "library";

pub struct MediaModule;

#[async_trait]
impl Module for MediaModule {
    fn name(&self) -> &'static str {
        "media"
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
        vec![(
            "admin/media.html",
            include_str!("../templates/admin/media.html"),
        )]
    }

    fn admin_menu(&self) -> Vec<MenuItem> {
        vec![MenuItem {
            section: "Content",
            label: "Media library",
            href: "/admin/media",
            permission: "media.view",
            icon: "fa-regular fa-images",
            order: 2,
            match_children: false,
        }]
    }
}
