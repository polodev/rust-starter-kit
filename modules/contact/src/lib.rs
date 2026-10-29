//! Contact messages: a public `/contact` form that submits without reloading the page (a small
//! server-rendered "component") protected by a honeypot, and an admin inbox where each message
//! opens in a modal and can be marked as replied.

mod commands;
mod controllers;
mod entities;
mod migrations;

use async_trait::async_trait;
use loco_modular::{MenuItem, Module};
use loco_rs::{controller::Routes, task::Tasks};
use sea_orm_migration::MigrationTrait;

pub use entities::contacts;

pub struct ContactModule;

#[async_trait]
impl Module for ContactModule {
    fn name(&self) -> &'static str {
        "contact"
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
                "contact/page.html",
                include_str!("../templates/contact/page.html"),
            ),
            (
                "contact/_form.html",
                include_str!("../templates/contact/_form.html"),
            ),
            (
                "admin/contacts.html",
                include_str!("../templates/admin/contacts.html"),
            ),
            (
                "admin/contact_modal.html",
                include_str!("../templates/admin/contact_modal.html"),
            ),
            (
                "admin/contact_show.html",
                include_str!("../templates/admin/contact_show.html"),
            ),
        ]
    }

    fn admin_menu(&self) -> Vec<MenuItem> {
        vec![MenuItem {
            section: "Content",
            label: "Contacts",
            href: "/admin/contacts",
            permission: "contacts.view",
            icon: "fa-regular fa-envelope",
            order: 3,
            match_children: true,
        }]
    }
}
