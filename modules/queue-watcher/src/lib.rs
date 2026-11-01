//! A queue monitor. It shows the history of the app's own background jobs (recorded with [`track`]), the
//! waiting and failed jobs of a Laravel queue that shares the database (`jobs`, `failed_jobs`), and lets staff
//! delete them. Screens are under `/admin/queue-watcher`; `queue.view` reads, `queue.manage` changes.

mod commands;
mod controllers;
mod database_queue;
mod entities;
mod migrations;
mod tracker;

use async_trait::async_trait;
use loco_modular::{MenuItem, Module, ScheduledJob};
use loco_rs::{controller::Routes, task::Tasks};
use sea_orm_migration::MigrationTrait;

pub use database_queue::{
    database_job, dispatch, flush_failed, retry_failed, run_worker, spawn_embedded_worker,
    work_once, DatabaseJob, JobsFailed, JobsFlushFailed, JobsRetryFailed, JobsWork,
    PendingDispatch,
};
pub use entities::{failed_jobs, jobs, queue_jobs};
pub use tracker::{mark_stale, purge, queued, stats, track, track_queued, JobInfo, Stats};

/// History older than this is removed by `queue-watcher:purge`.
pub const RETENTION_DAYS: i64 = 14;
/// A job running longer than this is marked stale by `queue-watcher:mark-stale`.
pub const STALE_AFTER_HOURS: i64 = 1;

pub struct QueueWatcherModule;

#[async_trait]
impl Module for QueueWatcherModule {
    fn name(&self) -> &'static str {
        "queue-watcher"
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
        vec![
            ScheduledJob::command("queue-watcher:mark-stale")
                .hourly()
                .tag("maintenance"),
            ScheduledJob::command("queue-watcher:purge")
                .daily_at(3, 40)
                .tag("maintenance"),
        ]
    }

    fn templates(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "admin/qw_tabs.html",
                include_str!("../templates/admin/qw_tabs.html"),
            ),
            (
                "admin/qw_overview.html",
                include_str!("../templates/admin/qw_overview.html"),
            ),
            (
                "admin/qw_jobs.html",
                include_str!("../templates/admin/qw_jobs.html"),
            ),
            (
                "admin/qw_job_show.html",
                include_str!("../templates/admin/qw_job_show.html"),
            ),
            (
                "admin/qw_pending.html",
                include_str!("../templates/admin/qw_pending.html"),
            ),
            (
                "admin/qw_pending_show.html",
                include_str!("../templates/admin/qw_pending_show.html"),
            ),
            (
                "admin/qw_failed.html",
                include_str!("../templates/admin/qw_failed.html"),
            ),
            (
                "admin/qw_failed_show.html",
                include_str!("../templates/admin/qw_failed_show.html"),
            ),
        ]
    }

    fn admin_menu(&self) -> Vec<MenuItem> {
        vec![MenuItem {
            section: "System",
            label: "Queue monitor",
            href: "/admin/queue-watcher",
            permission: "queue.view",
            icon: "fa-solid fa-list-check",
            order: 3,
            match_children: true,
        }]
    }
}
