//! Runtime contract every module crate implements, plus helpers that merge modules into a Loco app.

use std::time::Duration;

use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    bgworker::Queue,
    controller::{AppRoutes, Routes},
    task::Tasks,
    Result,
};
use sea_orm_migration::MigrationTrait;
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;

/// One admin sidebar link contributed by a module. The admin layout shows it only to people who
/// hold `permission`, groups it under `section`, and also lists it in the Ctrl/Cmd+K quick search.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MenuItem {
    pub section: &'static str,
    pub label: &'static str,
    pub href: &'static str,
    pub permission: &'static str,
    /// Font Awesome classes, e.g. `fa-solid fa-users`.
    pub icon: &'static str,
    /// Lower numbers come first inside a section.
    pub order: i32,
    /// Highlight this item for every path below `href` too.
    pub match_children: bool,
}

/// Section titles in display order; sections not listed here come last, alphabetically.
pub const MENU_SECTIONS: [&str; 4] = ["Overview", "Content", "People and access", "System"];

/// A self-contained feature crate living under `modules/<name>`.
#[async_trait]
pub trait Module: Send + Sync {
    /// Kebab-case module name, e.g. `role-permission`.
    fn name(&self) -> &'static str;

    /// Routes mounted into the application.
    fn routes(&self) -> Vec<Routes> {
        Vec::new()
    }

    /// Migrations merged by name into the application migrator.
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }

    /// Tera templates as `(name, source)`, usually built with `include_str!`.
    fn templates(&self) -> Vec<(&'static str, &'static str)> {
        Vec::new()
    }

    /// Commands registered here run as `cargo run -- task <signature> key:value`.
    fn register_tasks(&self, _tasks: &mut Tasks) {}

    /// Admin sidebar links this module adds.
    fn admin_menu(&self) -> Vec<MenuItem> {
        Vec::new()
    }

    /// Last chance to wrap the finished router, for example with a middleware layer.
    fn apply(&self, router: axum::Router, _ctx: &AppContext) -> axum::Router {
        router
    }

    /// Scheduled commands, the equivalent of Laravel's `Schedule` in `Console\Kernel`.
    fn schedule(&self) -> Vec<ScheduledJob> {
        Vec::new()
    }

    async fn connect_workers(&self, _ctx: &AppContext, _queue: &Queue) -> Result<()> {
        Ok(())
    }

    /// Idempotent seeder; safe to run repeatedly.
    async fn seed(&self, _ctx: &AppContext) -> Result<()> {
        Ok(())
    }
}

/// One scheduled command. Build it with [`ScheduledJob::command`] and a frequency helper:
///
/// ```ignore
/// ScheduledJob::command("pages:check-translations").daily_at(3, 0).tag("content")
/// ```
///
/// Cron fields are `sec min hour day-of-month month day-of-week`.
#[derive(Debug, Clone)]
pub struct ScheduledJob {
    pub name: String,
    /// Task signature plus optional arguments, e.g. `pages:check-translations strict:true`.
    pub run: String,
    pub cron: String,
    pub tags: Vec<String>,
    pub run_on_start: bool,
}

impl ScheduledJob {
    /// A task to run. Defaults to once a minute until a frequency is chosen.
    pub fn command(run: &str) -> Self {
        let signature = run.split_whitespace().next().unwrap_or(run);
        Self {
            name: signature.replace(':', "-"),
            run: run.to_string(),
            cron: "0 * * * * *".to_string(),
            tags: Vec::new(),
            run_on_start: false,
        }
    }

    pub fn named(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn cron(mut self, expression: &str) -> Self {
        self.cron = expression.to_string();
        self
    }

    pub fn every_minute(self) -> Self {
        self.cron("0 * * * * *")
    }

    pub fn every_five_minutes(self) -> Self {
        self.cron("0 */5 * * * *")
    }

    pub fn every_fifteen_minutes(self) -> Self {
        self.cron("0 */15 * * * *")
    }

    pub fn hourly(self) -> Self {
        self.cron("0 0 * * * *")
    }

    pub fn daily_at(self, hour: u8, minute: u8) -> Self {
        self.cron(&format!("0 {minute} {hour} * * *"))
    }

    pub fn weekly_on(self, day_of_week: u8, hour: u8, minute: u8) -> Self {
        self.cron(&format!("0 {minute} {hour} * * {day_of_week}"))
    }

    pub fn tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn run_on_start(mut self) -> Self {
        self.run_on_start = true;
        self
    }
}

/// Adds every module's scheduled jobs to the scheduler section of the loaded config.
/// Jobs written in `config/*.yaml` stay; a module job never replaces one with the same name.
///
/// # Errors
/// Fails if two modules declare the same job name.
pub fn merge_schedule(
    config: &mut loco_rs::config::Config,
    modules: &[Box<dyn Module>],
) -> Result<()> {
    use loco_rs::scheduler::{Config as SchedulerConfig, Job};

    let mut declared: Vec<(&'static str, ScheduledJob)> = Vec::new();
    for module in modules {
        for job in module.schedule() {
            if let Some((other, _)) = declared
                .iter()
                .find(|(_, existing)| existing.name == job.name)
            {
                return Err(loco_rs::Error::string(&format!(
                    "scheduled job `{}` is declared by both `{other}` and `{}`",
                    job.name,
                    module.name()
                )));
            }
            declared.push((module.name(), job));
        }
    }
    if declared.is_empty() {
        return Ok(());
    }

    let scheduler = config.scheduler.get_or_insert_with(|| SchedulerConfig {
        jobs: Default::default(),
        output: Default::default(),
    });
    for (_, job) in declared {
        scheduler.jobs.entry(job.name).or_insert(Job {
            run: job.run,
            shell: false,
            run_on_start: job.run_on_start,
            cron: job.cron,
            tags: if job.tags.is_empty() {
                None
            } else {
                Some(job.tags)
            },
            output: None,
        });
    }
    Ok(())
}

/// Merge routes of every module into one `AppRoutes`.
pub fn collect_routes(modules: &[Box<dyn Module>]) -> AppRoutes {
    modules
        .iter()
        .fold(AppRoutes::empty(), |app_routes, module| {
            app_routes.add_routes(module.routes())
        })
}

/// Admin menu of every module, ordered by section then `order` then label.
pub fn collect_menu(modules: &[Box<dyn Module>]) -> Vec<MenuItem> {
    let section_rank = |section: &str| {
        MENU_SECTIONS
            .iter()
            .position(|known| *known == section)
            .unwrap_or(MENU_SECTIONS.len())
    };
    let mut items: Vec<MenuItem> = modules
        .iter()
        .flat_map(|module| module.admin_menu())
        .collect();
    items.sort_by(|a, b| {
        section_rank(a.section)
            .cmp(&section_rank(b.section))
            .then_with(|| a.section.cmp(b.section))
            .then_with(|| a.order.cmp(&b.order))
            .then_with(|| a.label.cmp(b.label))
    });
    items
}

/// Let every module wrap the router.
pub fn apply_all(
    modules: &[Box<dyn Module>],
    mut router: axum::Router,
    ctx: &AppContext,
) -> axum::Router {
    for module in modules {
        router = module.apply(router, ctx);
    }
    router
}

/// Collect migrations of every module.
pub fn collect_migrations(modules: &[Box<dyn Module>]) -> Vec<Box<dyn MigrationTrait>> {
    modules
        .iter()
        .flat_map(|module| module.migrations())
        .collect()
}

/// Collect Tera templates of every module.
pub fn collect_templates(modules: &[Box<dyn Module>]) -> Vec<(&'static str, &'static str)> {
    modules
        .iter()
        .flat_map(|module| module.templates())
        .collect()
}

pub fn register_tasks(modules: &[Box<dyn Module>], tasks: &mut Tasks) {
    for module in modules {
        module.register_tasks(tasks);
    }
}

pub async fn connect_workers(
    modules: &[Box<dyn Module>],
    ctx: &AppContext,
    queue: &Queue,
) -> Result<()> {
    for module in modules {
        module.connect_workers(ctx, queue).await?;
    }
    Ok(())
}

pub async fn seed_all(modules: &[Box<dyn Module>], ctx: &AppContext) -> Result<()> {
    for module in modules {
        module.seed(ctx).await?;
    }
    Ok(())
}

/// Short-lived cache, the equivalent of Laravel's `Cache::remember`.
///
/// Returns the cached value for `key`, or runs `compute`, stores the result for `ttl` and returns it.
/// If the cache itself fails (for example the null driver is configured), the value is computed
/// directly so a cache problem never takes a page down. Errors from `compute` are always returned.
pub async fn remember<T, F>(ctx: &AppContext, key: &str, ttl: Duration, compute: F) -> Result<T>
where
    T: Serialize + DeserializeOwned + Send + Sync,
    F: Future<Output = Result<T>> + Send,
{
    if let Ok(Some(cached)) = ctx.cache.get::<T>(key).await {
        return Ok(cached);
    }
    let value = compute.await?;
    if let Err(error) = ctx.cache.insert_with_expiry(key, &value, ttl).await {
        tracing::debug!(%error, key, "cache write skipped");
    }
    Ok(value)
}

/// MariaDB/MySQL-safe YAML fixture loader.
///
/// Loco's own `db::seed` resets auto-increment counters with Postgres/SQLite-only SQL and returns
/// `BackendNotSupported` on MySQL. MariaDB advances `AUTO_INCREMENT` past explicit ids by itself,
/// so inserting the rows is all that is needed.
///
/// # Errors
/// Fails when the file cannot be read or parsed, or when the insert fails.
pub async fn seed_yaml<A>(db: &sea_orm::DatabaseConnection, path: &str) -> Result<()>
where
    <<A as sea_orm::ActiveModelTrait>::Entity as sea_orm::EntityTrait>::Model:
        sea_orm::IntoActiveModel<A>,
    for<'de> <<A as sea_orm::ActiveModelTrait>::Entity as sea_orm::EntityTrait>::Model:
        serde::Deserialize<'de>,
    A: sea_orm::ActiveModelTrait + Send + Sync,
    sea_orm::Insert<A>: Send + Sync,
{
    use sea_orm::{EntityTrait, IntoActiveModel};

    let bytes = tokio::fs::read(path).await?;
    let rows: Vec<serde_json::Value> =
        serde_yaml_ng::from_slice(&bytes).map_err(|e| loco_rs::Error::string(&e.to_string()))?;
    let mut models = Vec::with_capacity(rows.len());
    for row in rows {
        let model: <A::Entity as EntityTrait>::Model = serde_json::from_value(row)?;
        models.push(model.into_active_model());
    }
    if !models.is_empty() {
        <A::Entity as EntityTrait>::insert_many(models)
            .exec(db)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(&'static str, Vec<MenuItem>);

    #[async_trait]
    impl Module for Fake {
        fn name(&self) -> &'static str {
            self.0
        }
        fn admin_menu(&self) -> Vec<MenuItem> {
            self.1.clone()
        }
    }

    fn item(section: &'static str, label: &'static str, order: i32) -> MenuItem {
        MenuItem {
            section,
            label,
            href: "/x",
            permission: "a.view",
            icon: "fa-solid fa-circle",
            order,
            match_children: false,
        }
    }

    #[test]
    fn menu_is_ordered_by_section_then_order_then_label() {
        let modules: Vec<Box<dyn Module>> = vec![
            Box::new(Fake(
                "a",
                vec![
                    item("System", "Jobs", 1),
                    item("Custom", "Zed", 1),
                    item("Overview", "Dashboard", 1),
                ],
            )),
            Box::new(Fake(
                "b",
                vec![
                    item("Overview", "Alerts", 1),
                    item("People and access", "Users", 1),
                ],
            )),
        ];
        let labels: Vec<_> = collect_menu(&modules).iter().map(|i| i.label).collect();
        assert_eq!(labels, ["Alerts", "Dashboard", "Users", "Jobs", "Zed"]);
    }

    #[test]
    fn frequency_helpers_build_six_field_cron() {
        assert_eq!(
            ScheduledJob::command("a:b").daily_at(3, 0).cron,
            "0 0 3 * * *"
        );
        assert_eq!(
            ScheduledJob::command("a:b").every_five_minutes().cron,
            "0 */5 * * * *"
        );
        assert_eq!(
            ScheduledJob::command("a:b").weekly_on(1, 9, 30).cron,
            "0 30 9 * * 1"
        );
    }

    #[test]
    fn job_name_comes_from_the_signature() {
        assert_eq!(
            ScheduledJob::command("pages:check-translations strict:true").name,
            "pages-check-translations"
        );
    }
}
