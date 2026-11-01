//! `GET /admin/activity-log` (list with filters), `GET /admin/activity-log/{id}` (one entry with its changes),
//! `POST /admin/activity-log/{id}/delete` and `POST /admin/activity-log/clean` (older than N days).
//! Viewing needs `activity_logs.view`; deleting needs `activity_logs.manage`.

use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use loco_activity_log::activity_log;
use loco_rs::prelude::*;
use module_admin_layout::AdminPage;
use module_auth::{same_origin, users};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::DEFAULT_RETENTION_DAYS;

const PER_PAGE: u64 = 50;

#[derive(Deserialize, Default)]
struct ListQuery {
    log: Option<String>,
    event: Option<String>,
    q: Option<String>,
    page: Option<u64>,
    saved: Option<String>,
    n: Option<u64>,
    error: Option<String>,
}

/// Where the thing an entry was about can be opened, when this app has a page for it.
fn subject_link(kind: &str, id: &str) -> Option<String> {
    match kind {
        "user" => Some(format!("/admin/users/{id}")),
        "page" => Some(format!("/admin/pages/{id}")),
        "contact" => Some(format!("/admin/contacts/{id}")),
        _ => None,
    }
}

/// `user id -> name` for the causers of a set of entries.
async fn user_names(
    ctx: &AppContext,
    items: &[activity_log::Model],
) -> Result<HashMap<i64, String>> {
    let ids: Vec<i64> = items
        .iter()
        .filter(|item| item.causer_type.as_deref() == Some("user"))
        .filter_map(|item| item.causer_id.as_deref()?.parse().ok())
        .collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    Ok(users::Entity::find()
        .filter(users::Column::Id.is_in(ids))
        .all(&ctx.db)
        .await?
        .into_iter()
        .map(|user| (user.id, user.name))
        .collect())
}

fn causer_label(item: &activity_log::Model, names: &HashMap<i64, String>) -> Value {
    let (Some(kind), Some(id)) = (&item.causer_type, &item.causer_id) else {
        return json!({ "label": "System", "url": null });
    };
    if kind == "user" {
        if let Some(name) = id.parse::<i64>().ok().and_then(|id| names.get(&id)) {
            return json!({ "label": name, "url": format!("/admin/users/{id}") });
        }
    }
    json!({ "label": format!("{kind} #{id}"), "url": subject_link(kind, id) })
}

fn subject_label(item: &activity_log::Model) -> Value {
    let (Some(kind), Some(id)) = (&item.subject_type, &item.subject_id) else {
        return Value::Null;
    };
    json!({ "label": format!("{kind} #{id}"), "url": subject_link(kind, id) })
}

fn row(item: &activity_log::Model, names: &HashMap<i64, String>) -> Value {
    json!({
        "id": item.id, "log": item.log_name, "event": item.event, "description": item.description,
        "when": item.created_at.map(|at| at.format("%Y-%m-%d %H:%M:%S").to_string()),
        "causer": causer_label(item, names), "subject": subject_label(item),
    })
}

fn flash(query: &ListQuery) -> Option<Value> {
    let (kind, text) = match (query.saved.as_deref(), query.error.as_deref()) {
        (Some("deleted"), _) => ("ok", "Entry deleted.".to_string()),
        (Some("cleaned"), _) => (
            "ok",
            format!(
                "{} old entr{} deleted.",
                query.n.unwrap_or(0),
                if query.n == Some(1) { "y" } else { "ies" }
            ),
        ),
        (_, Some("days")) => (
            "error",
            "Enter a number of days between 1 and 36500.".to_string(),
        ),
        _ => return None,
    };
    Some(json!({ "kind": kind, "text": text }))
}

#[debug_handler]
async fn index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Query(query): Query<ListQuery>,
) -> Result<Response> {
    if !page.can("activity_logs.view") {
        return page.forbidden(&v);
    }
    let log = query.log.clone().filter(|value| !value.is_empty());
    let event = query
        .event
        .clone()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let search = query
        .q
        .clone()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let mut select = activity_log::Entity::find();
    if let Some(log) = &log {
        select = select.filter(activity_log::Column::LogName.eq(log.as_str()));
    }
    if let Some(event) = &event {
        select = select.filter(activity_log::Column::Event.eq(event.as_str()));
    }
    if let Some(search) = &search {
        select = select.filter(loco_database::contains_ignoring_case(
            activity_log::Column::Description,
            search.as_str(),
        ));
    }
    let paginator = select
        .order_by_desc(activity_log::Column::Id)
        .paginate(&ctx.db, PER_PAGE);
    let totals = paginator.num_items_and_pages().await?;
    let current = query
        .page
        .unwrap_or(1)
        .clamp(1, totals.number_of_pages.max(1));
    let items = paginator.fetch_page(current - 1).await?;
    let names = user_names(&ctx, &items).await?;
    let rows: Vec<Value> = items.iter().map(|item| row(item, &names)).collect();

    let log_names: Vec<String> = activity_log::Entity::find()
        .select_only()
        .column(activity_log::Column::LogName)
        .distinct()
        .order_by_asc(activity_log::Column::LogName)
        .into_tuple::<Option<String>>()
        .all(&ctx.db)
        .await?
        .into_iter()
        .flatten()
        .collect();

    // The filters stay in the pager links.
    let mut carried = Vec::new();
    for (key, value) in [("log", &log), ("event", &event), ("q", &search)] {
        if let Some(value) = value {
            carried.push(format!("{key}={}", urlencode(value)));
        }
    }
    page.render(
        &v,
        "admin/activity_log.html",
        json!({
            "title": "Activity log", "rows": rows, "total": totals.number_of_items,
            "log_names": log_names, "log": log, "event": event, "q": search,
            "page": current, "pages": totals.number_of_pages.max(1),
            "prev": if current > 1 { Some(current - 1) } else { None },
            "next": if current < totals.number_of_pages { Some(current + 1) } else { None },
            "carried": carried.join("&"),
            "status": flash(&query), "can_manage": page.can("activity_logs.manage"),
            "retention_days": DEFAULT_RETENTION_DAYS,
        }),
    )
}

fn urlencode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// `(attribute, old, new)` rows from the `attributes` / `old` properties, as Spatie writes them.
fn change_rows(properties: &Value) -> Vec<Value> {
    let Some(attributes) = properties.get("attributes").and_then(Value::as_object) else {
        return Vec::new();
    };
    let old = properties.get("old").and_then(Value::as_object);
    attributes
        .iter()
        .map(|(key, new)| {
            let shown = |value: &Value| match value {
                Value::Null => "null".to_string(),
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            json!({
                "attribute": key,
                "old": old.and_then(|map| map.get(key)).map(shown),
                "new": shown(new),
            })
        })
        .collect()
}

#[debug_handler]
async fn show(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Path(id): Path<loco_database::UnsignedBigInt>,
) -> Result<Response> {
    if !page.can("activity_logs.view") {
        return page.forbidden(&v);
    }
    let item = activity_log::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    let names = user_names(&ctx, std::slice::from_ref(&item)).await?;
    let properties = item.properties_json();
    let pretty = if properties.is_null() {
        String::new()
    } else {
        serde_json::to_string_pretty(&properties).unwrap_or_default()
    };
    let mut data = row(&item, &names);
    if let Some(map) = data.as_object_mut() {
        map.insert("batch".into(), json!(item.batch_uuid));
        map.insert("subject_type".into(), json!(item.subject_type));
        map.insert("causer_type".into(), json!(item.causer_type));
    }
    page.render(
        &v,
        "admin/activity_show.html",
        json!({
            "title": format!("Activity #{}", item.id), "item": data,
            "changes": change_rows(&properties), "properties": pretty,
            "can_manage": page.can("activity_logs.manage"),
        }),
    )
}

#[debug_handler]
async fn delete(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(id): Path<loco_database::UnsignedBigInt>,
) -> Result<Response> {
    if !page.can("activity_logs.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    let result = activity_log::Entity::delete_by_id(id).exec(&ctx.db).await?;
    if result.rows_affected == 0 {
        return Err(Error::NotFound);
    }
    Ok(Redirect::to("/admin/activity-log?saved=deleted").into_response())
}

#[derive(Deserialize)]
struct CleanForm {
    days: String,
}

#[debug_handler]
async fn clean(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Form(form): Form<CleanForm>,
) -> Result<Response> {
    if !page.can("activity_logs.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    let Some(days) = form
        .days
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|days| (1..=36_500).contains(days))
    else {
        return Ok(Redirect::to("/admin/activity-log?error=days").into_response());
    };
    let removed = loco_activity_log::clean(&ctx.db, days, None).await?;
    Ok(Redirect::to(&format!("/admin/activity-log?saved=cleaned&n={removed}")).into_response())
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new()
        .add("/admin/activity-log", get(index))
        .add("/admin/activity-log/clean", post(clean))
        .add("/admin/activity-log/{id}", get(show))
        .add("/admin/activity-log/{id}/delete", post(delete))]
}
