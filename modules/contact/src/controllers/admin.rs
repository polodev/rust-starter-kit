//! The admin inbox, RESTful: `GET /admin/contacts` (list), `GET /admin/contacts/{id}` (one message,
//! as the modal's HTML for the component script or as a page) and `POST /admin/contacts/{id}/status`
//! (mark replied or not replied).

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use chrono::Utc;
use loco_rs::prelude::*;
use module_admin_layout::AdminPage;
use module_auth::same_origin;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::Deserialize;
use serde_json::{json, Value};

use super::wants_fragment;
use crate::entities::contacts;

#[derive(Deserialize)]
struct ListQuery {
    status: Option<String>,
    q: Option<String>,
}

fn row(item: &contacts::Model) -> Value {
    json!({
        "id": item.id, "name": item.name, "email": item.email,
        "subject": item.subject, "message": item.message, "status": item.status,
        "replied": item.status == contacts::STATUS_REPLIED,
        "received": item.created_at.format("%Y-%m-%d %H:%M").to_string(),
        "replied_at": item.replied_at.map(|at| at.format("%Y-%m-%d %H:%M").to_string()),
        "locale": item.locale, "ip_address": item.ip_address,
    })
}

#[debug_handler]
async fn index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Query(query): Query<ListQuery>,
) -> Result<Response> {
    if !page.can("contacts.view") {
        return page.forbidden(&v);
    }
    let status = query
        .status
        .filter(|s| matches!(s.as_str(), contacts::STATUS_NEW | contacts::STATUS_REPLIED));
    let mut select = contacts::Entity::find().order_by_desc(contacts::Column::Id);
    if let Some(status) = &status {
        select = select.filter(contacts::Column::Status.eq(status.as_str()));
    }
    let needle = query.q.clone().unwrap_or_default().trim().to_lowercase();
    let rows: Vec<Value> = select
        .limit(500)
        .all(&ctx.db)
        .await?
        .iter()
        .filter(|item| {
            needle.is_empty()
                || item.name.to_lowercase().contains(&needle)
                || item.email.to_lowercase().contains(&needle)
                || item
                    .subject
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&needle)
                || item.message.to_lowercase().contains(&needle)
        })
        .map(row)
        .collect();
    let waiting = contacts::Entity::find()
        .filter(contacts::Column::Status.eq(contacts::STATUS_NEW))
        .all(&ctx.db)
        .await?
        .len();
    page.render(
        &v,
        "admin/contacts.html",
        json!({ "title": "Contacts", "contacts": rows, "filter": status, "q": query.q.unwrap_or_default(), "waiting": waiting, "can_manage": page.can("contacts.manage") }),
    )
}

async fn find(ctx: &AppContext, id: i64) -> Result<contacts::Model> {
    contacts::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)
}

#[debug_handler]
async fn show(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(id): Path<i64>,
) -> Result<Response> {
    if !page.can("contacts.view") {
        return page.forbidden(&v);
    }
    let item = find(&ctx, id).await?;
    let data = json!({ "item": row(&item), "can_manage": page.can("contacts.manage"), "title": item.name });
    if wants_fragment(&headers) {
        return Ok(Html(v.render("admin/contact_modal.html", data)?).into_response());
    }
    page.render(&v, "admin/contact_show.html", data)
}

#[derive(Deserialize)]
struct StatusForm {
    status: String,
}

#[debug_handler]
async fn set_status(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(id): Path<i64>,
    Form(form): Form<StatusForm>,
) -> Result<Response> {
    if !page.can("contacts.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    if !matches!(
        form.status.as_str(),
        contacts::STATUS_NEW | contacts::STATUS_REPLIED
    ) {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, "unknown status").into_response());
    }
    let item = find(&ctx, id).await?;
    let replied = form.status == contacts::STATUS_REPLIED;
    let old_status = item.status.clone();
    let new_status = form.status.clone();
    let mut active: contacts::ActiveModel = item.into();
    active.status = Set(form.status);
    active.replied_at = Set(replied.then(|| Utc::now().fixed_offset()));
    active.updated_at = Set(Utc::now().fixed_offset());
    let saved = active.update(&ctx.db).await?;
    loco_activity_log::activity(&ctx.db)
        .in_log("contacts")
        .caused_by("user", page.user.id)
        .performed_on("contact", id)
        .event("status_changed")
        .with_changes(loco_activity_log::changes(
            &json!({ "status": old_status }),
            &json!({ "status": new_status }),
            &[],
        ))
        .log_quietly(if replied {
            "Marked the message as replied"
        } else {
            "Marked the message as not replied"
        })
        .await;

    if wants_fragment(&headers) {
        let data = json!({ "item": row(&saved), "can_manage": true, "saved": true });
        return Ok(Html(v.render("admin/contact_modal.html", data)?).into_response());
    }
    Ok(Redirect::to(&format!("/admin/contacts/{id}")).into_response())
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new()
        .add("/admin/contacts", get(index))
        .add("/admin/contacts/{id}", get(show))
        .add("/admin/contacts/{id}/status", post(set_status))]
}
