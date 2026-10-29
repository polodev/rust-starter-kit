use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Result,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

use crate::entities::{permission_role, permissions, roles};

pub struct ListRoles;

#[async_trait]
impl Task for ListRoles {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "roles:list".to_string(),
            detail: "List roles with their permission counts.\nUsage: cargo run -- task roles:list"
                .to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
        let all = roles::Entity::find()
            .filter(roles::Column::IsActive.eq(true))
            .order_by_asc(roles::Column::Rank)
            .all(&ctx.db)
            .await?;
        if all.is_empty() {
            println!("no roles yet; run: cargo run -- task roles:sync");
        }
        for role in all {
            let ids: Vec<i64> = permission_role::Entity::find()
                .filter(permission_role::Column::RoleId.eq(role.id))
                .all(&ctx.db)
                .await?
                .into_iter()
                .map(|row| row.permission_id)
                .collect();
            let count = permissions::Entity::find()
                .filter(permissions::Column::Id.is_in(ids))
                .all(&ctx.db)
                .await?
                .len();
            println!(
                "{:<22} rank {}  {:>3} permissions  {}",
                role.slug, role.rank, count, role.name
            );
        }
        Ok(())
    }
}
