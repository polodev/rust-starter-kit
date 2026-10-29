mod access;
mod commands;
mod controllers;
mod definitions;
mod entities;
mod migrations;
mod sync;

use async_trait::async_trait;
use loco_modular::Module;
use loco_rs::{app::AppContext, controller::Routes, task::Tasks, Result};
use sea_orm_migration::MigrationTrait;

pub use access::{
    access_for, assign_role, grant_permission, revoke_permission, revoke_role, Access, RoleInfo,
};
pub use definitions::{permission_definitions, role_definitions, PermissionDef, RoleDef};
pub use entities::{permission_role, permission_user, permissions, role_user, roles};
pub use sync::{sync, SyncReport};

/// Permission that opens the admin dashboard and shows its link to the person.
pub const DASHBOARD_VIEW: &str = "dashboard.view";

pub struct RolePermissionModule;

#[async_trait]
impl Module for RolePermissionModule {
    fn name(&self) -> &'static str {
        "role-permission"
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

    async fn seed(&self, ctx: &AppContext) -> Result<()> {
        sync::sync(&ctx.db).await.map(|_| ())
    }
}
