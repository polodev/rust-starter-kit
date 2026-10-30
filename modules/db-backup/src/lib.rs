//! Database backups, like Spatie's `backup:run --only-db` but for the database only (no files). A dump
//! (`mysqldump`, gzip) is stored on one or more disks that are configured separately from the website's own
//! storage, so backups can live in another bucket on another account. See the README for the settings.
//!
//! | Command | What it does |
//! |---|---|
//! | `backup:run only-db:true` | dump and store (options: `disk:`, `filename:`) |
//! | `backup:list` | the backups on each destination |
//! | `backup:clean` | delete old ones by the retention settings (`dry-run:true` to preview) |
//! | `backup:monitor` | fail when a destination has no recent backup |

mod commands;
mod config;
mod disks;
mod dump;
mod retention;
mod service;

use async_trait::async_trait;
use loco_modular::{Module, ScheduledJob};
use loco_rs::task::Tasks;

pub use config::{BackupConfig, DiskConfig, Retention};
pub use dump::{parse_database_uri, Connection};
pub use retention::{to_delete, Backup};
pub use service::{clean, list, monitor, run, Cleaned, Health, Stored};

pub struct DbBackupModule;

#[async_trait]
impl Module for DbBackupModule {
    fn name(&self) -> &'static str {
        "db-backup"
    }

    fn register_tasks(&self, tasks: &mut Tasks) {
        commands::register(tasks);
    }

    /// The same order Spatie's docs suggest: clean, back up, then check. Each does nothing, and says so,
    /// until `settings.db_backup` exists.
    fn schedule(&self) -> Vec<ScheduledJob> {
        vec![
            ScheduledJob::command("backup:clean scheduled:true")
                .daily_at(1, 0)
                .tag("backup"),
            ScheduledJob::command("backup:run only-db:true scheduled:true")
                .daily_at(1, 30)
                .tag("backup"),
            ScheduledJob::command("backup:monitor scheduled:true")
                .daily_at(3, 0)
                .tag("backup"),
        ]
    }
}
