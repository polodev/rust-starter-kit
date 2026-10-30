//! The AI API logs screens under `/admin/ai-api-logs`:
//!
//! | Path | Does |
//! |---|---|
//! | `GET /` | list (filter by status, provider, kind, text; page length 25 to 500) |
//! | `GET /{id}` | one call: prompt, reply, tokens, cost, error |
//! | `POST /delete` | delete the selected rows, or every row that matches the filters |
//! | `POST /purge` | delete AI API logs older than N days |
//! | `POST /{id}/delete` | delete one |
//!
//! Reading needs `ai_api_logs.view`; every change needs `ai_api_logs.manage`.

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::Form as MultiForm;
use chrono::{Duration, Utc};
use loco_rs::prelude::*;
use module_admin_layout::AdminPage;
use module_auth::same_origin;
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Select,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{entities::ai_api_logs, Provider};

const BASE: &str = "/admin/ai-api-logs";
const PAGE_LENGTHS: [u64; 5] = [25, 50, 100, 200, 500];
const DEFAULT_PAGE_LENGTH: u64 = 25;
/// Rows deleted in one statement; MySQL limits the size of an `IN (...)` list.
const DELETE_CHUNK: usize = 500;

#[derive(Deserialize, Default)]
struct AiApiLogsListQuery {
    status: Option<String>,
    provider: Option<String>,
    kind: Option<String>,
    q: Option<String>,
    page: Option<u64>,
    per_page: Option<u64>,
    saved: Option<String>,
    n: Option<u64>,
    error: Option<String>,
}

struct AiApiLogFilters {
    status: Option<String>,
    provider: Option<String>,
    kind: Option<String>,
    search: Option<String>,
}

fn clean(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}

impl AiApiLogFilters {
    fn from_query(query: &AiApiLogsListQuery) -> Self {
        Self {
            status: clean(&query.status).filter(|s| ai_api_logs::STATUSES.contains(&s.as_str())),
            provider: clean(&query.provider)
                .filter(|p| Provider::parse(p).is_some())
                .map(|p| Provider::parse(&p).map_or(p, |p| p.name().to_string())),
            kind: clean(&query.kind).filter(|k| ["text", "vision", "image"].contains(&k.as_str())),
            search: clean(&query.q),
        }
    }

    fn apply(&self, mut select: Select<ai_api_logs::Entity>) -> Select<ai_api_logs::Entity> {
        if let Some(status) = &self.status {
            select = select.filter(ai_api_logs::Column::Status.eq(status.as_str()));
        }
        if let Some(provider) = &self.provider {
            select = select.filter(ai_api_logs::Column::Provider.eq(provider.as_str()));
        }
        if let Some(kind) = &self.kind {
            select = select.filter(ai_api_logs::Column::Kind.eq(kind.as_str()));
        }
        if let Some(search) = &self.search {
            select = select.filter(
                Condition::any()
                    .add(ai_api_logs::Column::Purpose.contains(search.as_str()))
                    .add(ai_api_logs::Column::Model.contains(search.as_str()))
                    .add(ai_api_logs::Column::Error.contains(search.as_str()))
                    .add(ai_api_logs::Column::Prompt.contains(search.as_str())),
            );
        }
        select
    }

    fn query_string(&self) -> String {
        let mut parts = Vec::new();
        for (key, value) in [
            ("status", &self.status),
            ("provider", &self.provider),
            ("kind", &self.kind),
            ("q", &self.search),
        ] {
            if let Some(value) = value {
                parts.push(format!("{key}={}", value.replace(' ', "+")));
            }
        }
        parts.join("&")
    }
}

macro_rules! require {
    ($page:expr, $v:expr, $permission:expr) => {
        if !$page.can($permission) {
            return $page.forbidden(&$v);
        }
    };
}

macro_rules! require_change {
    ($page:expr, $v:expr, $headers:expr) => {
        require!($page, $v, "ai_api_logs.manage");
        if !same_origin(&$headers) {
            return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
        }
    };
}

fn flash(query: &AiApiLogsListQuery) -> Option<Value> {
    let count = query.n.unwrap_or(0);
    let (kind, text) = match (query.saved.as_deref(), query.error.as_deref()) {
        (Some("deleted"), _) => ("ok", format!("{count} AI API log(s) deleted.")),
        (Some("purged"), _) => ("ok", format!("{count} old AI API log(s) removed.")),
        (_, Some("none")) => ("error", "Select at least one AI API log first.".to_string()),
        (_, Some("number")) => (
            "error",
            "Enter a whole number of days between 1 and 3650.".to_string(),
        ),
        _ => return None,
    };
    Some(json!({ "kind": kind, "text": text }))
}

fn time(at: Option<chrono::DateTime<chrono::FixedOffset>>) -> Option<String> {
    at.map(|at| at.format("%Y-%m-%d %H:%M:%S").to_string())
}

fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .chars()
        .take(140)
        .collect()
}

fn ai_api_log_row(item: &ai_api_logs::Model) -> Value {
    json!({
        "id": item.id, "kind": item.kind, "provider": item.provider, "model": item.model,
        "purpose": item.purpose, "status": item.status, "http_status": item.http_status,
        "duration_ms": item.duration_ms, "tokens": item.input_tokens + item.output_tokens,
        "cost": item.cost.map(|cost| format!("{cost:.4}")), "sensitive": item.sensitive,
        "error": item.error.as_deref().map(first_line), "created": time(item.created_at),
    })
}

#[debug_handler]
async fn ai_api_logs_index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Query(query): Query<AiApiLogsListQuery>,
) -> Result<Response> {
    require!(page, v, "ai_api_logs.view");
    let filters = AiApiLogFilters::from_query(&query);
    let per_page = query
        .per_page
        .filter(|n| PAGE_LENGTHS.contains(n))
        .unwrap_or(DEFAULT_PAGE_LENGTH);
    let paginator = filters
        .apply(ai_api_logs::Entity::find())
        .order_by_desc(ai_api_logs::Column::Id)
        .paginate(&ctx.db, per_page);
    let totals = paginator.num_items_and_pages().await?;
    let current = query
        .page
        .unwrap_or(1)
        .clamp(1, totals.number_of_pages.max(1));
    let rows: Vec<Value> = paginator
        .fetch_page(current - 1)
        .await?
        .iter()
        .map(ai_api_log_row)
        .collect();

    let mut carried = filters.query_string();
    if per_page != DEFAULT_PAGE_LENGTH {
        if !carried.is_empty() {
            carried.push('&');
        }
        carried.push_str(&format!("per_page={per_page}"));
    }
    let failed = ai_api_logs::Entity::find()
        .filter(ai_api_logs::Column::Status.eq(ai_api_logs::FAILED))
        .count(&ctx.db)
        .await?;
    page.render(
        &v,
        "admin/ai_api_logs_index.html",
        json!({
            "title": "AI API logs", "rows": rows, "total": totals.number_of_items, "failed_total": failed,
            "status": flash(&query), "can_manage": page.can("ai_api_logs.manage"),
            "filter_status": filters.status, "filter_provider": filters.provider, "filter_kind": filters.kind,
            "q": filters.search, "statuses": ai_api_logs::STATUSES, "providers": Provider::ALL.map(Provider::name),
            "kinds": ["text", "vision", "image"], "per_page": per_page, "page_lengths": PAGE_LENGTHS,
            "carried": carried, "page": current, "pages": totals.number_of_pages.max(1),
            "prev": if current > 1 { Some(current - 1) } else { None },
            "next": if current < totals.number_of_pages { Some(current + 1) } else { None },
            "retention_days": crate::RETENTION_DAYS,
        }),
    )
}

#[debug_handler]
async fn ai_api_log_show(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Path(id): Path<u64>,
) -> Result<Response> {
    require!(page, v, "ai_api_logs.view");
    let item = ai_api_logs::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    let mut data = ai_api_log_row(&item);
    if let Some(map) = data.as_object_mut() {
        map.insert("input_tokens".into(), json!(item.input_tokens));
        map.insert("output_tokens".into(), json!(item.output_tokens));
        map.insert("image_count".into(), json!(item.image_count));
        map.insert("prompt".into(), json!(item.prompt));
        map.insert("response".into(), json!(item.response));
        map.insert("full_error".into(), json!(item.error));
    }
    page.render(
        &v,
        "admin/ai_api_log_show.html",
        json!({ "title": format!("AI API log #{id}"), "item": data, "can_manage": page.can("ai_api_logs.manage") }),
    )
}

#[derive(Deserialize, Default)]
struct AiApiLogsDeleteForm {
    #[serde(default)]
    ids: Vec<u64>,
    /// `matching`: every row that fits the filters, not only the ones on this page.
    #[serde(default)]
    scope: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    q: String,
    #[serde(default)]
    return_to: String,
}

fn back(form: &AiApiLogsDeleteForm, suffix: &str) -> Response {
    // Only a query string is taken from the form, so the redirect can never leave this screen.
    let carried: String = form
        .return_to
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || "=&+_-".contains(*c))
        .collect();
    let mut url = format!("{BASE}?{suffix}");
    if !carried.is_empty() {
        url.push('&');
        url.push_str(&carried);
    }
    Redirect::to(&url).into_response()
}

#[debug_handler]
async fn ai_api_logs_delete_selected(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    MultiForm(form): MultiForm<AiApiLogsDeleteForm>,
) -> Result<Response> {
    require_change!(page, v, headers);
    let removed = if form.scope == "matching" {
        let filters = AiApiLogFilters::from_query(&AiApiLogsListQuery {
            status: Some(form.status.clone()),
            provider: Some(form.provider.clone()),
            kind: Some(form.kind.clone()),
            q: Some(form.q.clone()),
            ..AiApiLogsListQuery::default()
        });
        let mut condition = Condition::all();
        if let Some(status) = &filters.status {
            condition = condition.add(ai_api_logs::Column::Status.eq(status.as_str()));
        }
        if let Some(provider) = &filters.provider {
            condition = condition.add(ai_api_logs::Column::Provider.eq(provider.as_str()));
        }
        if let Some(kind) = &filters.kind {
            condition = condition.add(ai_api_logs::Column::Kind.eq(kind.as_str()));
        }
        if let Some(search) = &filters.search {
            condition = condition.add(
                Condition::any()
                    .add(ai_api_logs::Column::Purpose.contains(search.as_str()))
                    .add(ai_api_logs::Column::Model.contains(search.as_str()))
                    .add(ai_api_logs::Column::Error.contains(search.as_str()))
                    .add(ai_api_logs::Column::Prompt.contains(search.as_str())),
            );
        }
        ai_api_logs::Entity::delete_many()
            .filter(condition)
            .exec(&ctx.db)
            .await?
            .rows_affected
    } else {
        if form.ids.is_empty() {
            return Ok(back(&form, "error=none"));
        }
        let mut removed = 0;
        for chunk in form.ids.chunks(DELETE_CHUNK) {
            removed += ai_api_logs::Entity::delete_many()
                .filter(ai_api_logs::Column::Id.is_in(chunk.to_vec()))
                .exec(&ctx.db)
                .await?
                .rows_affected;
        }
        removed
    };
    Ok(back(&form, &format!("saved=deleted&n={removed}")))
}

#[debug_handler]
async fn ai_api_log_delete(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(id): Path<u64>,
) -> Result<Response> {
    require_change!(page, v, headers);
    if ai_api_logs::Entity::delete_by_id(id)
        .exec(&ctx.db)
        .await?
        .rows_affected
        == 0
    {
        return Err(Error::NotFound);
    }
    Ok(Redirect::to(&format!("{BASE}?saved=deleted&n=1")).into_response())
}

#[derive(Deserialize)]
struct AiApiLogsPurgeForm {
    #[serde(default)]
    days: String,
}

#[debug_handler]
async fn ai_api_logs_purge_older_than(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    axum::Form(form): axum::Form<AiApiLogsPurgeForm>,
) -> Result<Response> {
    require_change!(page, v, headers);
    let Some(days) = form
        .days
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|d| (1..=3650).contains(d))
    else {
        return Ok(Redirect::to(&format!("{BASE}?error=number")).into_response());
    };
    let removed = remove_ai_api_logs_older_than(&ctx.db, days).await?;
    Ok(Redirect::to(&format!("{BASE}?saved=purged&n={removed}")).into_response())
}

/// Deletes AI API logs created more than `days` days ago; the scheduled `ai-service:purge-ai-api-logs` uses it too.
pub async fn remove_ai_api_logs_older_than(
    db: &sea_orm::DatabaseConnection,
    days: i64,
) -> Result<u64> {
    let cutoff = (Utc::now() - Duration::days(days)).fixed_offset();
    Ok(ai_api_logs::Entity::delete_many()
        .filter(ai_api_logs::Column::CreatedAt.lt(cutoff))
        .exec(db)
        .await?
        .rows_affected)
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new()
        .add(BASE, get(ai_api_logs_index))
        .add(&format!("{BASE}/delete"), post(ai_api_logs_delete_selected))
        .add(&format!("{BASE}/purge"), post(ai_api_logs_purge_older_than))
        .add(&format!("{BASE}/{{id}}"), get(ai_api_log_show))
        .add(&format!("{BASE}/{{id}}/delete"), post(ai_api_log_delete))]
}
