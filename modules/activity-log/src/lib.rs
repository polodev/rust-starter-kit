//! Admin screens for the activity log (`/admin/activity-log`) and the `activitylog:clean` command. Writing
//! entries is done with the `loco-activity-log` package (`activity(&db)...log("...")`), which any module can use.

mod commands;
mod controllers;
mod migrations;

use async_trait::async_trait;
use loco_modular::{MenuItem, Module, ScheduledJob};
use loco_rs::{controller::Routes, task::Tasks};
use sea_orm_migration::MigrationTrait;

pub use loco_activity_log::{activity, changes, created, deleted, DEFAULT_LOG};

/// Like Spatie's `delete_records_older_than_days`.
pub const DEFAULT_RETENTION_DAYS: i64 = 365;

pub struct ActivityLogModule;

#[async_trait]
impl Module for ActivityLogModule {
    fn name(&self) -> &'static str {
        "activity-log"
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
        vec![ScheduledJob::command("activitylog:clean")
            .daily_at(3, 20)
            .tag("maintenance")]
    }

    fn templates(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "admin/activity_log.html",
                include_str!("../templates/admin/activity_log.html"),
            ),
            (
                "admin/activity_show.html",
                include_str!("../templates/admin/activity_show.html"),
            ),
        ]
    }

    fn admin_menu(&self) -> Vec<MenuItem> {
        vec![MenuItem {
            section: "System",
            label: "Activity log",
            href: "/admin/activity-log",
            permission: "activity_logs.view",
            icon: "fa-regular fa-rectangle-list",
            order: 2,
            match_children: true,
        }]
    }
}
