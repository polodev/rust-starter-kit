//! The role and permission screens: an overview,
//! roles (with a page per role), permissions, the permission matrix, user assignments (with search
//! and an edit page) and backend links, all under `/admin/role-permissions`.
//! Roles and permissions themselves are defined in code and copied into the database by "Sync".

use std::collections::{BTreeMap, HashMap, HashSet};

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Extension,
};
use loco_rs::prelude::*;
use module_auth::{same_origin, users};
use module_role_permission::{
    access_for, assign_role, grant_permission, permission_role, permission_user, permissions,
    revoke_permission, revoke_role, role_user, roles, sync,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::page::{AdminMenu, AdminPage};

const BASE: &str = "/admin/role-permissions";

#[derive(Deserialize, Default)]
struct Flash {
    saved: Option<String>,
    error: Option<String>,
    q: Option<String>,
}

fn flash(query: &Flash) -> Option<Value> {
    let (kind, text) = match (query.saved.as_deref(), query.error.as_deref()) {
        (Some("synced"), _) => ("ok", "Roles and permissions were synchronized from code."),
        (Some("assignment"), _) => ("ok", "Assignment saved."),
        (_, Some("self")) => (
            "error",
            "You cannot change your own role or permissions here.",
        ),
        (_, Some("role")) => ("error", "That role does not exist."),
        (_, Some("permission")) => (
            "error",
            "One of those permissions cannot be granted directly.",
        ),
        _ => return None,
    };
    Some(json!({ "kind": kind, "text": text }))
}

async fn active_roles(ctx: &AppContext) -> Result<Vec<roles::Model>> {
    Ok(roles::Entity::find()
        .filter(roles::Column::IsActive.eq(true))
        .order_by_asc(roles::Column::Rank)
        .all(&ctx.db)
        .await?)
}

async fn active_permissions(ctx: &AppContext) -> Result<Vec<permissions::Model>> {
    Ok(permissions::Entity::find()
        .filter(permissions::Column::IsActive.eq(true))
        .order_by_asc(permissions::Column::Key)
        .all(&ctx.db)
        .await?)
}

/// `role id -> permission ids` for every role.
async fn grants(ctx: &AppContext) -> Result<HashMap<i64, HashSet<i64>>> {
    let mut map: HashMap<i64, HashSet<i64>> = HashMap::new();
    for link in permission_role::Entity::find().all(&ctx.db).await? {
        map.entry(link.role_id)
            .or_default()
            .insert(link.permission_id);
    }
    Ok(map)
}

fn shell(page: &AdminPage, active: &str, extra: Value) -> Value {
    let mut data = json!({
        "rp_active": active,
        "can_sync": page.can("roles.sync"),
        "can_assign": page.can("roles.assign"),
    });
    if let (Some(base), Value::Object(extra)) = (data.as_object_mut(), extra) {
        base.extend(extra);
    }
    data
}

macro_rules! require {
    ($page:expr, $v:expr, $permission:expr) => {
        if !$page.can($permission) {
            return $page.forbidden(&$v);
        }
    };
}

#[debug_handler]
async fn overview(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Query(query): Query<Flash>,
) -> Result<Response> {
    require!(page, v, "roles.view");
    let role_count = roles::Entity::find()
        .filter(roles::Column::IsActive.eq(true))
        .count(&ctx.db)
        .await?;
    let permission_count = permissions::Entity::find()
        .filter(permissions::Column::IsActive.eq(true))
        .count(&ctx.db)
        .await?;
    let assigned = role_user::Entity::find().count(&ctx.db).await?;
    page.render(
        &v,
        "admin/rp_overview.html",
        shell(
            &page,
            "overview",
            json!({
                "title": "Advanced authorization overview", "status": flash(&query),
                "role_count": role_count, "permission_count": permission_count, "assigned_count": assigned,
            }),
        ),
    )
}

#[debug_handler]
async fn sync_now(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
) -> Result<Response> {
    require!(page, v, "roles.sync");
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    sync(&ctx.db).await?;
    // Permissions of the signed-in person may have changed; the next request reloads them.
    let _ = access_for(&ctx.db, page.user.id).await?;
    Ok(Redirect::to(&format!("{BASE}?saved=synced")).into_response())
}

#[debug_handler]
async fn roles_index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
) -> Result<Response> {
    require!(page, v, "roles.view");
    let all = active_roles(&ctx).await?;
    let granted = grants(&ctx).await?;
    let mut rows = Vec::new();
    for role in &all {
        let holders = role_user::Entity::find()
            .filter(role_user::Column::RoleId.eq(role.id))
            .count(&ctx.db)
            .await?;
        rows.push(json!({
            "id": role.id, "rank": role.rank, "name": role.name, "slug": role.slug,
            "description": role.description,
            "permissions": granted.get(&role.id).map_or(0, HashSet::len), "holders": holders,
        }));
    }
    page.render(
        &v,
        "admin/rp_roles.html",
        shell(&page, "roles", json!({ "title": "Roles", "roles": rows })),
    )
}

#[debug_handler]
async fn role_show(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Path(role_id): Path<i64>,
) -> Result<Response> {
    require!(page, v, "roles.view");
    let role = roles::Entity::find_by_id(role_id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    let granted = grants(&ctx).await?;
    let ids = granted.get(&role.id).cloned().unwrap_or_default();
    let permission_rows: Vec<Value> = active_permissions(&ctx)
        .await?
        .into_iter()
        .filter(|permission| ids.contains(&permission.id))
        .map(|permission| json!({ "key": permission.key, "name": permission.name, "group": permission.group_name }))
        .collect();
    let holder_ids: Vec<i64> = role_user::Entity::find()
        .filter(role_user::Column::RoleId.eq(role.id))
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|link| link.user_id)
        .collect();
    let holders: Vec<Value> = users::Entity::find()
        .filter(users::Column::Id.is_in(holder_ids))
        .order_by_asc(users::Column::Name)
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|user| json!({ "id": user.id, "name": user.name, "email": user.email }))
        .collect();
    page.render(
        &v,
        "admin/rp_role_show.html",
        shell(
            &page,
            "roles",
            json!({
                "title": role.name,
                "role": { "id": role.id, "name": role.name, "slug": role.slug, "rank": role.rank, "description": role.description },
                "permissions": permission_rows, "holders": holders,
            }),
        ),
    )
}

#[debug_handler]
async fn permissions_index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
) -> Result<Response> {
    require!(page, v, "roles.view");
    let all_roles = active_roles(&ctx).await?;
    let granted = grants(&ctx).await?;
    let rows: Vec<Value> = active_permissions(&ctx)
        .await?
        .into_iter()
        .map(|permission| {
            let held_by: Vec<&str> = all_roles
                .iter()
                .filter(|role| granted.get(&role.id).is_some_and(|ids| ids.contains(&permission.id)))
                .map(|role| role.name.as_str())
                .collect();
            json!({ "key": permission.key, "name": permission.name, "group": permission.group_name, "roles": held_by })
        })
        .collect();
    page.render(
        &v,
        "admin/rp_permissions.html",
        shell(
            &page,
            "permissions",
            json!({ "title": "Permissions", "permissions": rows }),
        ),
    )
}

#[debug_handler]
async fn matrix(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
) -> Result<Response> {
    require!(page, v, "roles.view");
    let all_roles = active_roles(&ctx).await?;
    let granted = grants(&ctx).await?;
    let columns: Vec<Value> = all_roles
        .iter()
        .map(|role| json!({ "id": role.id, "name": role.name }))
        .collect();
    let rows: Vec<Value> = active_permissions(&ctx)
        .await?
        .into_iter()
        .map(|permission| {
            let cells: Vec<bool> = all_roles
                .iter()
                .map(|role| granted.get(&role.id).is_some_and(|ids| ids.contains(&permission.id)))
                .collect();
            json!({ "key": permission.key, "name": permission.name, "group": permission.group_name, "cells": cells })
        })
        .collect();
    page.render(
        &v,
        "admin/rp_matrix.html",
        shell(
            &page,
            "matrix",
            json!({ "title": "Permission matrix", "columns": columns, "matrix": rows }),
        ),
    )
}

#[debug_handler]
async fn users_index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Query(query): Query<Flash>,
) -> Result<Response> {
    require!(page, v, "roles.view");
    let needle = query.q.clone().unwrap_or_default().trim().to_lowercase();
    let people: Vec<users::Model> = users::Entity::find()
        .order_by_asc(users::Column::Name)
        .all(&ctx.db)
        .await?
        .into_iter()
        .filter(|user| {
            needle.is_empty()
                || user.name.to_lowercase().contains(&needle)
                || user.email.to_lowercase().contains(&needle)
        })
        .collect();
    let role_names: HashMap<i64, String> = active_roles(&ctx)
        .await?
        .into_iter()
        .map(|role| (role.id, role.name))
        .collect();
    let role_of: HashMap<i64, i64> = role_user::Entity::find()
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|link| (link.user_id, link.role_id))
        .collect();
    let mut direct: HashMap<i64, usize> = HashMap::new();
    for link in permission_user::Entity::find().all(&ctx.db).await? {
        *direct.entry(link.user_id).or_default() += 1;
    }
    let rows: Vec<Value> = people
        .iter()
        .map(|user| {
            json!({
                "id": user.id, "name": user.name, "email": user.email,
                "role": role_of.get(&user.id).and_then(|id| role_names.get(id)),
                "direct": direct.get(&user.id).copied().unwrap_or(0), "is_you": user.id == page.user.id,
            })
        })
        .collect();
    page.render(
        &v,
        "admin/rp_users.html",
        shell(
            &page,
            "users",
            json!({ "title": "User assignments", "users": rows, "q": query.q.clone().unwrap_or_default(), "status": flash(&query) }),
        ),
    )
}

async fn user_edit_data(
    ctx: &AppContext,
    user: &users::Model,
    page: &AdminPage,
    query: &Flash,
) -> Result<Value> {
    let current = access_for(&ctx.db, user.id).await?;
    let direct_ids: HashSet<i64> = permission_user::Entity::find()
        .filter(permission_user::Column::UserId.eq(user.id))
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|link| link.permission_id)
        .collect();
    let role_options: Vec<Value> = active_roles(ctx)
        .await?
        .into_iter()
        .map(|role| json!({ "slug": role.slug, "name": role.name, "selected": current.has_role(&role.slug) }))
        .collect();
    // Role-management permissions come only from a role, never as a direct exception.
    let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for permission in active_permissions(ctx)
        .await?
        .into_iter()
        .filter(|p| !p.key.starts_with("roles."))
    {
        groups.entry(permission.group_name.clone()).or_default().push(json!({
            "key": permission.key, "name": permission.name, "checked": direct_ids.contains(&permission.id),
        }));
    }
    let permission_groups: Vec<Value> = groups
        .into_iter()
        .map(|(group, items)| json!({ "group": group, "items": items }))
        .collect();
    Ok(shell(
        page,
        "users",
        json!({
            "title": user.name, "status": flash(query),
            "person": { "id": user.id, "name": user.name, "email": user.email, "is_you": user.id == page.user.id },
            "role_options": role_options, "permission_groups": permission_groups,
        }),
    ))
}

#[debug_handler]
async fn user_edit(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Path(user_id): Path<i64>,
    Query(query): Query<Flash>,
) -> Result<Response> {
    require!(page, v, "roles.assign");
    let user = users::Entity::find_by_id(user_id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    let data = user_edit_data(&ctx, &user, &page, &query).await?;
    page.render(&v, "admin/rp_user_edit.html", data)
}

#[debug_handler]
async fn user_update(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(user_id): Path<i64>,
    body: Bytes,
) -> Result<Response> {
    require!(page, v, "roles.assign");
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    let back = |error: &str| {
        Redirect::to(&format!("{BASE}/users/{user_id}/edit?error={error}")).into_response()
    };
    // Nobody can lock themselves out by changing their own role or permissions.
    if user_id == page.user.id {
        return Ok(back("self"));
    }
    users::Entity::find_by_id(user_id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;

    let fields: Vec<(String, String)> = serde_urlencoded::from_bytes(&body).unwrap_or_default();
    let role = fields
        .iter()
        .find(|(name, _)| name == "role")
        .map(|(_, value)| value.as_str())
        .unwrap_or("");
    let wanted: HashSet<&str> = fields
        .iter()
        .filter(|(name, _)| name == "permissions")
        .map(|(_, value)| value.as_str())
        .collect();
    if wanted.iter().any(|key| key.starts_with("roles.")) {
        return Ok(back("permission"));
    }
    let known: HashSet<String> = active_permissions(&ctx)
        .await?
        .into_iter()
        .map(|p| p.key)
        .collect();
    if wanted.iter().any(|key| !known.contains(*key)) {
        return Ok(back("permission"));
    }
    if !role.is_empty() && !active_roles(&ctx).await?.iter().any(|r| r.slug == role) {
        return Ok(back("role"));
    }

    if role.is_empty() {
        revoke_role(&ctx.db, user_id).await?;
    } else {
        assign_role(&ctx.db, user_id, role).await?;
    }
    let by_id: HashMap<i64, String> = permissions::Entity::find()
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|permission| (permission.id, permission.key))
        .collect();
    let current: HashSet<String> = permission_user::Entity::find()
        .filter(permission_user::Column::UserId.eq(user_id))
        .all(&ctx.db)
        .await?
        .into_iter()
        .filter_map(|link| by_id.get(&link.permission_id).cloned())
        .collect();
    for key in wanted.iter().filter(|key| !current.contains(**key)) {
        grant_permission(&ctx.db, user_id, key).await?;
    }
    for key in current.iter().filter(|key| !wanted.contains(key.as_str())) {
        revoke_permission(&ctx.db, user_id, key).await?;
    }
    Ok(Redirect::to(&format!("{BASE}/users?saved=assignment")).into_response())
}

/// The admin pages and the permission each one needs: where a permission is actually used.
#[debug_handler]
async fn locations(
    ViewEngine(v): ViewEngine<TeraView>,
    page: AdminPage,
    Extension(menu): Extension<AdminMenu>,
    Query(query): Query<Flash>,
) -> Result<Response> {
    require!(page, v, "roles.view");
    let needle = query.q.clone().unwrap_or_default().trim().to_lowercase();
    let rows: Vec<Value> = menu
        .0
        .iter()
        .filter(|item| {
            needle.is_empty()
                || item.label.to_lowercase().contains(&needle)
                || item.href.to_lowercase().contains(&needle)
                || item.permission.to_lowercase().contains(&needle)
        })
        .map(|item| json!({ "section": item.section, "label": item.label, "href": item.href, "permission": item.permission }))
        .collect();
    page.render(
        &v,
        "admin/rp_locations.html",
        shell(
            &page,
            "locations",
            json!({ "title": "Backend links", "links": rows, "q": query.q.clone().unwrap_or_default() }),
        ),
    )
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new()
        .add(BASE, get(overview))
        .add(&format!("{BASE}/sync"), post(sync_now))
        .add(&format!("{BASE}/roles"), get(roles_index))
        .add(&format!("{BASE}/roles/{{id}}"), get(role_show))
        .add(&format!("{BASE}/permissions"), get(permissions_index))
        .add(&format!("{BASE}/permissions-with-role-info"), get(matrix))
        .add(&format!("{BASE}/users"), get(users_index))
        .add(&format!("{BASE}/users/{{id}}/edit"), get(user_edit))
        .add(&format!("{BASE}/users/{{id}}"), post(user_update))
        .add(&format!("{BASE}/locations"), get(locations))]
}
