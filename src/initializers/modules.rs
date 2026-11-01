use async_trait::async_trait;
use axum::{Extension, Router as AxumRouter};
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};
use module_admin_layout::AdminMenu;

/// Lets every module wrap the router (sessions, avatars, ...) and publishes the admin sidebar
/// that all modules contributed.
pub struct ModulesInitializer;

#[async_trait]
impl Initializer for ModulesInitializer {
    fn name(&self) -> String {
        "modules".to_string()
    }

    async fn after_routes(&self, router: AxumRouter, ctx: &AppContext) -> Result<AxumRouter> {
        let modules = modules_registry::all();
        let menu = AdminMenu(loco_modular::collect_menu(&modules));
        // Development: one process does everything. Production runs `mail:work` under Supervisor instead.
        module_mail::spawn_embedded_worker(ctx);
        module_queue_watcher::spawn_embedded_worker(
            ctx,
            loco_modular::collect_database_jobs(&modules),
        );
        let router = loco_modular::apply_all(&modules, router, ctx);
        Ok(router.layer(Extension(menu)))
    }
}
