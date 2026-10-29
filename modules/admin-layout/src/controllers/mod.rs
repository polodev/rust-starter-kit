use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use loco_rs::prelude::*;
use module_auth::{same_origin, sessions, users};
use module_role_permission::{
    access_for, assign_role, permissions, revoke_role, role_user, roles, sync,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;

use crate::page::AdminPage;

#[debug_handler]
async fn dashboard(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
) -> Result<Response> {
    if !page.can("dashboard.view") {
        return page.forbidden(&v);
    }
    let now = chrono::Utc::now().fixed_offset();
    let stats = json!([
        { "label": "Users", "value": users::Entity::find().count(&ctx.db).await? },
        { "label": "Staff with a role", "value": role_user::Entity::find().count(&ctx.db).await? },
        { "label": "Active roles", "value": roles::Entity::find().filter(roles::Column::IsActive.eq(true)).count(&ctx.db).await? },
        { "label": "Permissions", "value": permissions::Entity::find().filter(permissions::Column::IsActive.eq(true)).count(&ctx.db).await? },
        { "label": "Active sessions", "value": sessions::Entity::find().filter(sessions::Column::ExpiresAt.gt(now)).count(&ctx.db).await? },
    ]);

    let latest = users::Entity::find()
        .order_by_desc(users::Column::Id)
        .limit(5)
        .all(&ctx.db)
        .await?;
    let role_names = role_names_for(&ctx, latest.iter().map(|user| user.id)).await?;
    let recent: Vec<_> = latest
        .iter()
        .map(|user| json!({
            "id": user.id, "name": user.name, "email": user.email,
            "role": role_names.get(&user.id), "joined": user.created_at.format("%Y-%m-%d").to_string(),
        }))
        .collect();
    let link_count: usize = page.nav.iter().map(|section| section.links.len()).sum();
    page.render(
        &v,
        "admin/dashboard.html",
        json!({ "title": "Dashboard", "stats": stats, "recent": recent, "link_count": link_count }),
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
        json!({ "title": "Users", "users": rows, "role_options": role_options, "can_assign": page.can("roles.assign") }),
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
async fn roles_index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
) -> Result<Response> {
    if !page.can("roles.view") {
        return page.forbidden(&v);
    }
    let all_roles = roles::Entity::find()
        .filter(roles::Column::IsActive.eq(true))
        .order_by_asc(roles::Column::Rank)
        .all(&ctx.db)
        .await?;
    let all_permissions = permissions::Entity::find()
        .filter(permissions::Column::IsActive.eq(true))
        .order_by_asc(permissions::Column::Key)
        .all(&ctx.db)
        .await?;
    let links = module_role_permission::permission_role::Entity::find()
        .all(&ctx.db)
        .await?;

    let columns: Vec<_> = all_roles
        .iter()
        .map(|role| json!({ "name": role.name, "slug": role.slug }))
        .collect();
    let mut matrix = Vec::new();
    for permission in &all_permissions {
        let cells: Vec<bool> = all_roles
            .iter()
            .map(|role| {
                links
                    .iter()
                    .any(|link| link.role_id == role.id && link.permission_id == permission.id)
            })
            .collect();
        matrix.push(json!({ "key": permission.key, "name": permission.name, "group": permission.group_name, "cells": cells }));
    }
    let mut role_rows = Vec::new();
    for role in &all_roles {
        let holders = role_user::Entity::find()
            .filter(role_user::Column::RoleId.eq(role.id))
            .count(&ctx.db)
            .await?;
        let granted = links.iter().filter(|link| link.role_id == role.id).count();
        role_rows.push(json!({
            "rank": role.rank, "name": role.name, "slug": role.slug, "description": role.description,
            "permissions": granted, "holders": holders,
        }));
    }
    page.render(
        &v,
        "admin/roles.html",
        json!({ "title": "Roles and permissions", "roles": role_rows, "columns": columns, "matrix": matrix, "can_sync": page.can("roles.sync") }),
    )
}

#[debug_handler]
async fn roles_sync(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
) -> Result<Response> {
    if !page.can("roles.sync") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    sync(&ctx.db).await?;
    // Permissions of the signed-in person may have changed; the next request reloads them.
    let _ = access_for(&ctx.db, page.user.id).await?;
    Ok(Redirect::to("/admin/roles?synced=1").into_response())
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
    vec![Routes::new()
        .add("/admin", get(dashboard))
        .add("/admin/users", get(users_index))
        .add("/admin/users/{id}/role", post(assign_user_role))
        .add("/admin/roles", get(roles_index))
        .add("/admin/roles/sync", post(roles_sync))
        .add("/admin/schedule", get(schedule_index))]
}
