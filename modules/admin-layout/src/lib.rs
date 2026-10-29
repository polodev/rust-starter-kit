mod commands;
mod controllers;
mod migrations;
mod page;

use async_trait::async_trait;
use loco_modular::{MenuItem, Module};
use loco_rs::{controller::Routes, task::Tasks};
use sea_orm_migration::MigrationTrait;

pub use page::{AdminMenu, AdminPage};

pub struct AdminLayoutModule;

#[async_trait]
impl Module for AdminLayoutModule {
    fn name(&self) -> &'static str {
        "admin-layout"
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
                "layouts/admin.html",
                include_str!("../templates/layouts/admin.html"),
            ),
            (
                "admin/dashboard.html",
                include_str!("../templates/admin/dashboard.html"),
            ),
            (
                "admin/users.html",
                include_str!("../templates/admin/users.html"),
            ),
            (
                "admin/roles.html",
                include_str!("../templates/admin/roles.html"),
            ),
            (
                "admin/schedule.html",
                include_str!("../templates/admin/schedule.html"),
            ),
            (
                "admin/forbidden.html",
                include_str!("../templates/admin/forbidden.html"),
            ),
        ]
    }

    fn admin_menu(&self) -> Vec<MenuItem> {
        vec![
            MenuItem {
                section: "Overview",
                label: "Dashboard",
                href: "/admin",
                permission: module_role_permission::DASHBOARD_VIEW,
                icon: "fa-solid fa-table-cells-large",
                order: 1,
                match_children: false,
            },
            MenuItem {
                section: "People and access",
                label: "Users",
                href: "/admin/users",
                permission: "users.view",
                icon: "fa-solid fa-users",
                order: 1,
                match_children: true,
            },
            MenuItem {
                section: "People and access",
                label: "Roles and permissions",
                href: "/admin/roles",
                permission: "roles.view",
                icon: "fa-solid fa-user-shield",
                order: 2,
                match_children: true,
            },
            MenuItem {
                section: "System",
                label: "Scheduled jobs",
                href: "/admin/schedule",
                permission: "queue.view",
                icon: "fa-solid fa-clock-rotate-left",
                order: 1,
                match_children: false,
            },
        ]
    }
}
