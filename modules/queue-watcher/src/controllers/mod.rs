//! The queue monitor screens under `/admin/queue-watcher`:
//!
//! | Path | Shows |
//! |---|---|
//! | `/` | counts and the newest tracked jobs |
//! | `/jobs`, `/jobs/{id}` | tracked history (filter by status, queue, text); delete, purge, mark stale |
//! | `/pending`, `/pending/{id}` | waiting jobs of a Laravel queue (`jobs` table); delete |
//! | `/failed`, `/failed/{id}` | its failed jobs (`failed_jobs` table); delete, flush |
//!
//! Reading needs `queue.view`; every change needs `queue.manage`.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use chrono::{DateTime, Utc};
use loco_database::{UnsignedBigInt, UnsignedInt};
use loco_rs::prelude::*;
use module_admin_layout::AdminPage;
use module_auth::same_origin;
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    entities::{failed_jobs, jobs, queue_jobs},
    tracker, RETENTION_DAYS, STALE_AFTER_HOURS,
};

const BASE: &str = "/admin/queue-watcher";
const PER_PAGE: u64 = 25;

#[derive(Deserialize, Default)]
struct ListQuery {
    status: Option<String>,
    queue: Option<String>,
    q: Option<String>,
    page: Option<u64>,
    saved: Option<String>,
    n: Option<u64>,
    error: Option<String>,
}

fn flash(query: &ListQuery) -> Option<Value> {
    let count = query.n.unwrap_or(0);
    let (kind, text) = match (query.saved.as_deref(), query.error.as_deref()) {
        (Some("deleted"), _) => ("ok", "Deleted.".to_string()),
        (Some("purged"), _) => ("ok", format!("{count} old record(s) removed.")),
        (Some("stale"), _) => ("ok", format!("{count} job(s) marked as stale.")),
        (Some("flushed"), _) => ("ok", format!("{count} failed job(s) deleted.")),
        (_, Some("number")) => (
            "error",
            "Enter a whole number in the allowed range.".to_string(),
        ),
        _ => return None,
    };
    Some(json!({ "kind": kind, "text": text }))
}

fn shell(page: &AdminPage, active: &str, extra: Value) -> Value {
    let mut data = json!({ "qw_active": active, "can_manage": page.can("queue.manage") });
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

/// A state-changing request: needs `queue.manage` and a same-site origin.
macro_rules! require_change {
    ($page:expr, $v:expr, $headers:expr) => {
        require!($page, $v, "queue.manage");
        if !same_origin(&$headers) {
            return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
        }
    };
}

fn time(at: Option<DateTime<chrono::FixedOffset>>) -> Option<String> {
    at.map(|at| at.format("%Y-%m-%d %H:%M:%S").to_string())
}

fn unix_time(seconds: UnsignedInt) -> String {
    let seconds = loco_database::to_u32(seconds);
    DateTime::<Utc>::from_timestamp(i64::from(seconds), 0)
        .map(|at| at.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

/// The class of a Laravel job from its payload (`displayName`, else `job`).
fn payload_name(payload: &str) -> String {
    serde_json::from_str::<Value>(payload)
        .ok()
        .and_then(|value| {
            value
                .get("displayName")
                .or_else(|| value.get("job"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "(unknown job)".to_string())
}

fn pretty(text: &str) -> String {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| text.to_string())
}

fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(160)
        .collect()
}

fn pager(current: u64, pages: u64) -> Value {
    json!({
        "page": current, "pages": pages.max(1),
        "prev": if current > 1 { Some(current - 1) } else { None },
        "next": if current < pages { Some(current + 1) } else { None },
    })
}

fn job_row(item: &queue_jobs::Model) -> Value {
    json!({
        "id": item.id, "name": item.name, "queue": item.queue, "status": item.status,
        "attempt": item.attempt, "seconds": item.seconds(),
        "started": time(item.started_at), "finished": time(item.finished_at.or(item.failed_at)),
        "created": time(item.created_at),
        "error": item.exception_message.as_deref().map(first_line),
    })
}

#[debug_handler]
async fn overview(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Query(query): Query<ListQuery>,
) -> Result<Response> {
    require!(page, v, "queue.view");
    let stats = tracker::stats(&ctx.db).await?;
    let recent: Vec<Value> = queue_jobs::Entity::find()
        .order_by_desc(queue_jobs::Column::Id)
        .limit(10)
        .all(&ctx.db)
        .await?
        .iter()
        .map(job_row)
        .collect();
    page.render(
        &v,
        "admin/qw_overview.html",
        shell(
            &page,
            "overview",
            json!({
                "title": "Queue monitor", "status": flash(&query), "recent": recent,
                "stats": {
                    "queued": stats.queued, "running": stats.running, "finished": stats.finished,
                    "failed": stats.failed, "stale": stats.stale,
                    "pending": stats.pending, "failed_table": stats.failed_table,
                },
                "retention_days": RETENTION_DAYS, "stale_hours": STALE_AFTER_HOURS,
            }),
        ),
    )
}

#[debug_handler]
async fn jobs_index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Query(query): Query<ListQuery>,
) -> Result<Response> {
    require!(page, v, "queue.view");
    let status = query
        .status
        .clone()
        .filter(|status| queue_jobs::STATUSES.contains(&status.as_str()));
    let queue = query
        .queue
        .clone()
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty());
    let search = query
        .q
        .clone()
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty());

    let mut select = queue_jobs::Entity::find();
    if let Some(status) = &status {
        select = select.filter(queue_jobs::Column::Status.eq(status.as_str()));
    }
    if let Some(queue) = &queue {
        select = select.filter(queue_jobs::Column::Queue.eq(queue.as_str()));
    }
    if let Some(search) = &search {
        select = select.filter(
            Condition::any()
                .add(loco_database::contains_ignoring_case(
                    queue_jobs::Column::Name,
                    search.as_str(),
                ))
                .add(loco_database::contains_ignoring_case(
                    queue_jobs::Column::JobId,
                    search.as_str(),
                ))
                .add(loco_database::contains_ignoring_case(
                    queue_jobs::Column::JobClass,
                    search.as_str(),
                )),
        );
    }
    let paginator = select
        .order_by_desc(queue_jobs::Column::Id)
        .paginate(&ctx.db, PER_PAGE);
    let totals = paginator.num_items_and_pages().await?;
    let current = query
        .page
        .unwrap_or(1)
        .clamp(1, totals.number_of_pages.max(1));
    let rows: Vec<Value> = paginator
        .fetch_page(current - 1)
        .await?
        .iter()
        .map(job_row)
        .collect();

    let mut carried = Vec::new();
    for (key, value) in [("status", &status), ("queue", &queue), ("q", &search)] {
        if let Some(value) = value {
            carried.push(format!("{key}={}", value.replace(' ', "+")));
        }
    }
    let mut extra = json!({
        "title": "Tracked jobs", "rows": rows, "total": totals.number_of_items,
        "status": flash(&query), "filter_status": status, "queue": queue, "q": search,
        "statuses": queue_jobs::STATUSES, "carried": carried.join("&"),
        "retention_days": RETENTION_DAYS, "stale_hours": STALE_AFTER_HOURS,
    });
    if let (Some(map), Value::Object(pager)) = (
        extra.as_object_mut(),
        pager(current, totals.number_of_pages),
    ) {
        map.extend(pager);
    }
    page.render(&v, "admin/qw_jobs.html", shell(&page, "jobs", extra))
}

#[debug_handler]
async fn job_show(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Path(id): Path<UnsignedBigInt>,
) -> Result<Response> {
    require!(page, v, "queue.view");
    let item = queue_jobs::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    let mut data = job_row(&item);
    if let Some(map) = data.as_object_mut() {
        map.insert("job_id".into(), json!(item.job_id));
        map.insert("class".into(), json!(item.job_class));
        map.insert("description".into(), json!(item.job_description));
        map.insert("connection".into(), json!(item.connection));
        map.insert("max_tries".into(), json!(item.max_tries));
        map.insert("tags".into(), json!(item.tags));
        map.insert("exception".into(), json!(item.exception_message));
        map.insert("trace".into(), json!(item.exception_trace));
        map.insert("payload".into(), json!(item.payload.as_deref().map(pretty)));
        map.insert("failed".into(), json!(time(item.failed_at)));
    }
    page.render(
        &v,
        "admin/qw_job_show.html",
        shell(
            &page,
            "jobs",
            json!({ "title": format!("Job #{id}"), "item": data }),
        ),
    )
}

#[debug_handler]
async fn job_delete(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(id): Path<UnsignedBigInt>,
) -> Result<Response> {
    require_change!(page, v, headers);
    if queue_jobs::Entity::delete_by_id(id)
        .exec(&ctx.db)
        .await?
        .rows_affected
        == 0
    {
        return Err(Error::NotFound);
    }
    Ok(Redirect::to(&format!("{BASE}/jobs?saved=deleted")).into_response())
}

#[derive(Deserialize)]
struct NumberForm {
    #[serde(default)]
    days: String,
    #[serde(default)]
    hours: String,
}

#[debug_handler]
async fn jobs_purge(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Form(form): Form<NumberForm>,
) -> Result<Response> {
    require_change!(page, v, headers);
    let Some(days) = form
        .days
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|d| (1..=3650).contains(d))
    else {
        return Ok(Redirect::to(&format!("{BASE}/jobs?error=number")).into_response());
    };
    let removed = tracker::purge(&ctx.db, days).await?;
    Ok(Redirect::to(&format!("{BASE}/jobs?saved=purged&n={removed}")).into_response())
}

#[debug_handler]
async fn jobs_mark_stale(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Form(form): Form<NumberForm>,
) -> Result<Response> {
    require_change!(page, v, headers);
    let Some(hours) = form
        .hours
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|h| (1..=720).contains(h))
    else {
        return Ok(Redirect::to(&format!("{BASE}/jobs?error=number")).into_response());
    };
    let marked = tracker::mark_stale(&ctx.db, hours).await?;
    Ok(Redirect::to(&format!("{BASE}/jobs?saved=stale&n={marked}")).into_response())
}

#[debug_handler]
async fn pending_index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Query(query): Query<ListQuery>,
) -> Result<Response> {
    require!(page, v, "queue.view");
    let queue = query
        .queue
        .clone()
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty());
    let mut select = jobs::Entity::find();
    if let Some(queue) = &queue {
        select = select.filter(jobs::Column::Queue.eq(queue.as_str()));
    }
    let paginator = select
        .order_by_desc(jobs::Column::Id)
        .paginate(&ctx.db, PER_PAGE);
    let totals = paginator.num_items_and_pages().await?;
    let current = query
        .page
        .unwrap_or(1)
        .clamp(1, totals.number_of_pages.max(1));
    let rows: Vec<Value> = paginator
        .fetch_page(current - 1)
        .await?
        .iter()
        .map(|item| {
            json!({
                "id": item.id, "queue": item.queue, "name": payload_name(&item.payload),
                "attempts": item.attempts, "reserved": item.reserved_at.map(unix_time),
                "available": unix_time(item.available_at), "created": unix_time(item.created_at),
            })
        })
        .collect();
    let mut extra = json!({
        "title": "Pending jobs", "rows": rows, "total": totals.number_of_items,
        "status": flash(&query), "queue": queue,
        "carried": queue.as_ref().map(|q| format!("queue={}", q.replace(' ', "+"))).unwrap_or_default(),
    });
    if let (Some(map), Value::Object(pager)) = (
        extra.as_object_mut(),
        pager(current, totals.number_of_pages),
    ) {
        map.extend(pager);
    }
    page.render(&v, "admin/qw_pending.html", shell(&page, "pending", extra))
}

#[debug_handler]
async fn pending_show(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Path(id): Path<UnsignedBigInt>,
) -> Result<Response> {
    require!(page, v, "queue.view");
    let item = jobs::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    page.render(
        &v,
        "admin/qw_pending_show.html",
        shell(
            &page,
            "pending",
            json!({
                "title": format!("Pending job #{id}"),
                "item": {
                    "id": item.id, "queue": item.queue, "name": payload_name(&item.payload),
                    "attempts": item.attempts, "reserved": item.reserved_at.map(unix_time),
                    "available": unix_time(item.available_at), "created": unix_time(item.created_at),
                    "payload": pretty(&item.payload),
                },
            }),
        ),
    )
}

#[debug_handler]
async fn pending_delete(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(id): Path<UnsignedBigInt>,
) -> Result<Response> {
    require_change!(page, v, headers);
    if jobs::Entity::delete_by_id(id)
        .exec(&ctx.db)
        .await?
        .rows_affected
        == 0
    {
        return Err(Error::NotFound);
    }
    Ok(Redirect::to(&format!("{BASE}/pending?saved=deleted")).into_response())
}

fn failed_row(item: &failed_jobs::Model) -> Value {
    json!({
        "id": item.id, "uuid": item.uuid, "queue": item.queue, "connection": item.connection,
        "name": payload_name(&item.payload), "failed": time(Some(item.failed_at)),
        "error": first_line(&item.exception),
    })
}

#[debug_handler]
async fn failed_index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Query(query): Query<ListQuery>,
) -> Result<Response> {
    require!(page, v, "queue.view");
    let search = query
        .q
        .clone()
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty());
    let mut select = failed_jobs::Entity::find();
    if let Some(search) = &search {
        select = select.filter(
            Condition::any()
                .add(loco_database::contains_ignoring_case(
                    failed_jobs::Column::Uuid,
                    search.as_str(),
                ))
                .add(loco_database::contains_ignoring_case(
                    failed_jobs::Column::Queue,
                    search.as_str(),
                ))
                .add(loco_database::contains_ignoring_case(
                    failed_jobs::Column::Exception,
                    search.as_str(),
                ))
                .add(loco_database::contains_ignoring_case(
                    failed_jobs::Column::Payload,
                    search.as_str(),
                )),
        );
    }
    let paginator = select
        .order_by_desc(failed_jobs::Column::Id)
        .paginate(&ctx.db, PER_PAGE);
    let totals = paginator.num_items_and_pages().await?;
    let current = query
        .page
        .unwrap_or(1)
        .clamp(1, totals.number_of_pages.max(1));
    let rows: Vec<Value> = paginator
        .fetch_page(current - 1)
        .await?
        .iter()
        .map(failed_row)
        .collect();
    let mut extra = json!({
        "title": "Failed jobs", "rows": rows, "total": totals.number_of_items,
        "status": flash(&query), "q": search,
        "carried": search.as_ref().map(|q| format!("q={}", q.replace(' ', "+"))).unwrap_or_default(),
    });
    if let (Some(map), Value::Object(pager)) = (
        extra.as_object_mut(),
        pager(current, totals.number_of_pages),
    ) {
        map.extend(pager);
    }
    page.render(&v, "admin/qw_failed.html", shell(&page, "failed", extra))
}

#[debug_handler]
async fn failed_show(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Path(id): Path<UnsignedBigInt>,
) -> Result<Response> {
    require!(page, v, "queue.view");
    let item = failed_jobs::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    let mut data = failed_row(&item);
    if let Some(map) = data.as_object_mut() {
        map.insert("exception".into(), json!(item.exception));
        map.insert("payload".into(), json!(pretty(&item.payload)));
    }
    page.render(
        &v,
        "admin/qw_failed_show.html",
        shell(
            &page,
            "failed",
            json!({ "title": format!("Failed job #{id}"), "item": data }),
        ),
    )
}

#[debug_handler]
async fn failed_delete(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(id): Path<UnsignedBigInt>,
) -> Result<Response> {
    require_change!(page, v, headers);
    if failed_jobs::Entity::delete_by_id(id)
        .exec(&ctx.db)
        .await?
        .rows_affected
        == 0
    {
        return Err(Error::NotFound);
    }
    Ok(Redirect::to(&format!("{BASE}/failed?saved=deleted")).into_response())
}

#[debug_handler]
async fn failed_flush(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
) -> Result<Response> {
    require_change!(page, v, headers);
    let removed = failed_jobs::Entity::delete_many()
        .exec(&ctx.db)
        .await?
        .rows_affected;
    Ok(Redirect::to(&format!("{BASE}/failed?saved=flushed&n={removed}")).into_response())
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new()
        .add(BASE, get(overview))
        .add(&format!("{BASE}/jobs"), get(jobs_index))
        .add(&format!("{BASE}/jobs/purge"), post(jobs_purge))
        .add(&format!("{BASE}/jobs/mark-stale"), post(jobs_mark_stale))
        .add(&format!("{BASE}/jobs/{{id}}"), get(job_show))
        .add(&format!("{BASE}/jobs/{{id}}/delete"), post(job_delete))
        .add(&format!("{BASE}/pending"), get(pending_index))
        .add(&format!("{BASE}/pending/{{id}}"), get(pending_show))
        .add(
            &format!("{BASE}/pending/{{id}}/delete"),
            post(pending_delete),
        )
        .add(&format!("{BASE}/failed"), get(failed_index))
        .add(&format!("{BASE}/failed/flush"), post(failed_flush))
        .add(&format!("{BASE}/failed/{{id}}"), get(failed_show))
        .add(&format!("{BASE}/failed/{{id}}/delete"), post(failed_delete))]
}
