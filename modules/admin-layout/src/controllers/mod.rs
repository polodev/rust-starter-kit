use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use loco_rs::prelude::*;
use module_auth::{same_origin, users};
use module_role_permission::{assign_role, revoke_role, role_user, roles};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

use crate::page::AdminPage;

mod role_permissions;

#[debug_handler]
async fn dashboard(ViewEngine(v): ViewEngine<TeraView>, page: AdminPage) -> Result<Response> {
    if !page.can("dashboard.view") {
        return page.forbidden(&v);
    }
    let link_count: usize = page.nav.iter().map(|section| section.links.len()).sum();
    page.render(
        &v,
        "admin/dashboard.html",
        json!({ "title": "Dashboard", "link_count": link_count }),
    )
}

/// `user id -> role name` for a set of users.
async fn role_names_for(
    ctx: &AppContext,
    ids: impl Iterator<Item = i64>,
) -> Result<HashMap<i64, String>> {
    let ids: Vec<i64> = ids.collect();
    let links = role_user::Entity::find()
        .filter(role_user::Column::UserId.is_in(ids))
        .all(&ctx.db)
        .await?;
    let all_roles: HashMap<i64, String> = roles::Entity::find()
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|role| (role.id, role.name))
        .collect();
    Ok(links
        .into_iter()
        .filter_map(|link| {
            all_roles
                .get(&link.role_id)
                .map(|name| (link.user_id, name.clone()))
        })
        .collect())
}

#[debug_handler]
async fn users_index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
) -> Result<Response> {
    if !page.can("users.view") {
        return page.forbidden(&v);
    }
    let list = users::Entity::find()
        .order_by_desc(users::Column::Id)
        .limit(200)
        .all(&ctx.db)
        .await?;
    let role_names = role_names_for(&ctx, list.iter().map(|user| user.id)).await?;
    let role_slugs: HashMap<i64, String> = {
        let slugs: HashMap<i64, String> = roles::Entity::find()
            .all(&ctx.db)
            .await?
            .into_iter()
            .map(|role| (role.id, role.slug))
            .collect();
        role_user::Entity::find()
            .all(&ctx.db)
            .await?
            .into_iter()
            .filter_map(|link| {
                slugs
                    .get(&link.role_id)
                    .map(|slug| (link.user_id, slug.clone()))
            })
            .collect()
    };
    let rows: Vec<_> = list
        .iter()
        .map(|user| json!({
            "id": user.id, "name": user.name, "email": user.email,
            "role": role_names.get(&user.id), "role_slug": role_slugs.get(&user.id),
            "verified": user.email_verified_at.is_some(), "joined": user.created_at.format("%Y-%m-%d").to_string(),
            "is_you": user.id == page.user.id,
        }))
        .collect();
    let role_options: Vec<_> = roles::Entity::find()
        .filter(roles::Column::IsActive.eq(true))
        .order_by_asc(roles::Column::Rank)
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|role| json!({ "slug": role.slug, "name": role.name }))
        .collect();
    page.render(
        &v,
        "admin/users.html",
        json!({ "title": "Users", "users": rows, "role_options": role_options, "can_assign": page.can("roles.assign"), "can_manage": page.can("users.manage") }),
    )
}

#[derive(Deserialize)]
struct RoleForm {
    role: String,
}

#[debug_handler]
async fn assign_user_role(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(user_id): Path<i64>,
    Form(form): Form<RoleForm>,
) -> Result<Response> {
    if !page.can("roles.assign") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    // Nobody can lock themselves out by changing their own role.
    if user_id == page.user.id {
        return Ok(Redirect::to("/admin/users?error=self").into_response());
    }
    if users::Entity::find_by_id(user_id)
        .one(&ctx.db)
        .await?
        .is_none()
    {
        return Err(Error::NotFound);
    }
    if form.role.is_empty() {
        revoke_role(&ctx.db, user_id).await?;
    } else {
        assign_role(&ctx.db, user_id, &form.role).await?;
    }
    Ok(Redirect::to("/admin/users?saved=1").into_response())
}

#[debug_handler]
async fn schedule_index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
) -> Result<Response> {
    if !page.can("queue.view") {
        return page.forbidden(&v);
    }
    let mut jobs: Vec<_> = ctx
        .config
        .scheduler
        .as_ref()
        .map(|scheduler| {
            scheduler
                .jobs
                .iter()
                .map(|(name, job)| json!({ "name": name, "cron": job.cron, "run": job.run, "tags": job.tags.clone().unwrap_or_default() }))
                .collect()
        })
        .unwrap_or_default();
    jobs.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    page.render(
        &v,
        "admin/schedule.html",
        json!({ "title": "Scheduled jobs", "jobs": jobs }),
    )
}

pub fn routes() -> Vec<Routes> {
    let mut all = role_permissions::routes();
    all.push(
        Routes::new()
            .add("/admin", get(dashboard))
            .add("/admin/users", get(users_index))
            .add("/admin/users/{id}/role", post(assign_user_role))
            .add("/admin/schedule", get(schedule_index)),
    );
    all
}
