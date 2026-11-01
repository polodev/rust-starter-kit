use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Hooks, Initializer},
    bgworker::{BackgroundWorker, Queue},
    boot::{create_app, BootResult, StartMode},
    config::Config,
    controller::AppRoutes,
    db::truncate_table,
    environment::Environment,
    storage::{self, Storage},
    task::Tasks,
    Result,
};
use migration::Migrator;
use std::path::Path;

#[allow(unused_imports)]
use crate::{
    controllers, initializers, models::_entities::users, tasks, workers::downloader::DownloadWorker,
};

pub struct App;
#[async_trait]
impl Hooks for App {
    fn app_name() -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    fn app_version() -> String {
        format!(
            "{} ({})",
            env!("CARGO_PKG_VERSION"),
            option_env!("BUILD_SHA")
                .or(option_env!("GITHUB_SHA"))
                .unwrap_or("dev")
        )
    }

    async fn boot(
        mode: StartMode,
        environment: &Environment,
        config: Config,
    ) -> Result<BootResult> {
        // The test environment truncates and recreates its schema, so it may only ever touch a `*_test` database.
        if *environment == Environment::Test
            && !config
                .database
                .uri
                .split(['?', '#'])
                .next()
                .unwrap_or_default()
                .ends_with("_test")
        {
            return Err(loco_rs::Error::string(
                "refusing to boot the test environment: the database name must end in `_test`",
            ));
        }
        create_app::<Self, Migrator>(mode, environment, config).await
    }

    async fn load_config(env: &Environment) -> Result<Config> {
        let mut config = env.load()?;
        loco_modular::merge_schedule(&mut config, &modules_registry::all())?;
        Ok(config)
    }

    async fn after_context(ctx: AppContext) -> Result<AppContext> {
        // The build (MySQL/MariaDB by default, PostgreSQL with `--features postgres`) must match DATABASE_URL.
        loco_database::check_backend(ctx.db.get_database_backend())
            .map_err(|message| loco_rs::Error::string(&message))?;
        // Uploaded files: the local folder (storage/uploads) or the R2 public bucket (MEDIA_STORAGE=r2 in .env).
        // Tests keep them in memory.
        let driver = if ctx.environment == Environment::Test {
            storage::drivers::mem::new()
        } else {
            module_media::driver_from_environment()?
        };
        Ok(ctx
            .into_builder()
            .storage(Storage::single(driver).into())
            .build())
    }

    async fn initializers(_ctx: &AppContext) -> Result<Vec<Box<dyn Initializer>>> {
        Ok(vec![
            Box::new(initializers::view_engine::ViewEngineInitializer),
            Box::new(initializers::modules::ModulesInitializer),
        ])
    }

    fn routes(_ctx: &AppContext) -> AppRoutes {
        modules_registry::all().iter().fold(
            AppRoutes::with_default_routes().add_route(controllers::auth::routes()),
            |app_routes, module| app_routes.add_routes(module.routes()),
        )
    }

    async fn connect_workers(ctx: &AppContext, queue: &Queue) -> Result<()> {
        queue.register(DownloadWorker::build(ctx)).await?;
        loco_modular::connect_workers(&modules_registry::all(), ctx, queue).await
    }

    #[allow(unused_variables)]
    fn register_tasks(tasks: &mut Tasks) {
        // tasks-inject (do not remove)
        tasks.register(tasks::tinker::Tinker);
        tasks.register(tasks::user_create::UserCreate);
        tasks.register(tasks::user_delete::UserDelete);
        let modules = modules_registry::all();
        loco_modular::register_tasks(&modules, tasks);
        // `jobs:work` runs the database jobs that the modules declare.
        tasks.register(module_queue_watcher::JobsWork::new(
            loco_modular::collect_database_jobs(&modules),
        ));
    }

    async fn truncate(ctx: &AppContext) -> Result<()> {
        truncate_table(&ctx.db, users::Entity).await?;
        Ok(())
    }

    async fn seed(ctx: &AppContext, base: &Path) -> Result<()> {
        // Fixture users exist for the test suite only; real data is created with the `user_create` task.
        if ctx.environment == Environment::Test {
            loco_modular::seed_yaml::<users::ActiveModel>(
                &ctx.db,
                &base.join("users.yaml").display().to_string(),
            )
            .await?;
        }
        crate::seeders::demo_user::run_in_development(ctx).await?;
        loco_modular::seed_all(&modules_registry::all(), ctx).await
    }
}
