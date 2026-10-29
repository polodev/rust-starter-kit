//! Admin screens for pages: the list, and the edit page where the feature image lives.

use axum::{
    extract::{Multipart, Path},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use loco_rs::prelude::*;
use module_admin_layout::AdminPage;
use module_auth::same_origin;
use module_media::{read_form, InteractsWithMedia, MediaError};
use module_site_layout::SUPPORTED_LOCALES;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

use super::forget_cache;
use crate::{
    entities::{page_translations, pages},
    FEATURE_IMAGE,
};

/// Longest alternative text kept for an image.
const MAX_ALT_CHARS: usize = 160;

async fn find(ctx: &AppContext, id: i64) -> Result<pages::Model> {
    pages::Entity::find_by_id(id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)
}

fn rejected() -> Response {
    (StatusCode::FORBIDDEN, "request rejected").into_response()
}

fn back(id: i64, key: &str, value: &str) -> Response {
    Redirect::to(&format!("/admin/pages/{id}/edit?{key}={value}")).into_response()
}

#[debug_handler]
async fn index(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
) -> Result<Response> {
    if !page.can("pages.view") {
        return page.forbidden(&v);
    }
    let all = pages::Entity::find()
        .order_by_asc(pages::Column::SortOrder)
        .all(&ctx.db)
        .await?;
    let mut rows = Vec::new();
    for item in &all {
        let translations = page_translations::Entity::find()
            .filter(page_translations::Column::PageId.eq(item.id))
            .all(&ctx.db)
            .await?;
        let title = translations
            .iter()
            .find(|t| t.locale == "en")
            .or_else(|| translations.first())
            .map_or_else(|| item.slug.clone(), |t| t.title.clone());
        let languages: Vec<String> = translations
            .iter()
            .map(|t| t.locale.to_uppercase())
            .collect();
        rows.push(json!({
            "id": item.id, "slug": item.slug, "title": title, "published": item.is_published,
            "languages": languages, "image": item.get_first_media_url(&ctx, FEATURE_IMAGE).await?,
            "updated": item.updated_at.format("%Y-%m-%d").to_string(),
        }));
    }
    page.render(
        &v,
        "admin/pages.html",
        json!({ "title": "Pages", "rows": rows, "can_manage": page.can("pages.manage") }),
    )
}

#[derive(Deserialize, Default)]
struct EditQuery {
    saved: Option<String>,
    error: Option<String>,
}

const LANGUAGE_NAMES: [(&str, &str); 2] = [("en", "English"), ("bn", "বাংলা")];
const MAX_TITLE_CHARS: usize = 255;
const MAX_META_CHARS: usize = 320;
/// The body column is a TEXT column (64 KB); stay safely under it.
const MAX_BODY_BYTES: usize = 60_000;

fn language_name(code: &str) -> &'static str {
    LANGUAGE_NAMES
        .iter()
        .find(|(known, _)| *known == code)
        .map_or("Other", |(_, name)| name)
}

fn status_message(query: &EditQuery) -> Option<Value> {
    let (kind, text) = match (query.saved.as_deref(), query.error.as_deref()) {
        (Some("content"), _) => ("ok", "Page content saved."),
        (Some("image"), _) => ("ok", "Feature image saved."),
        (Some("removed"), _) => ("ok", "Feature image removed."),
        (Some("published"), _) => ("ok", "The page is now published."),
        (Some("unpublished"), _) => ("ok", "The page is now hidden from visitors."),
        (_, Some("empty")) => ("error", "Choose an image first."),
        (_, Some("type")) => ("error", "Upload a PNG, JPG, WebP or GIF image."),
        (_, Some("size")) => ("error", "The image is larger than 2 MB."),
        (_, Some(_)) => ("error", "The image could not be saved."),
        _ => return None,
    };
    Some(json!({ "kind": kind, "text": text }))
}

/// One entry per supported language, filled from the database; missing translations are empty.
async fn language_rows(ctx: &AppContext, page_id: i64) -> Result<Vec<Value>> {
    let existing = page_translations::Entity::find()
        .filter(page_translations::Column::PageId.eq(page_id))
        .all(&ctx.db)
        .await?;
    Ok(SUPPORTED_LOCALES
        .iter()
        .map(|code| {
            let found = existing
                .iter()
                .find(|translation| translation.locale == *code);
            json!({
                "code": code, "label": language_name(code), "exists": found.is_some(),
                "title": found.map(|t| t.title.clone()).unwrap_or_default(),
                "meta": found.map(|t| t.meta_description.clone()).unwrap_or_default(),
                "body": found.map(|t| t.body_html.clone()).unwrap_or_default(),
            })
        })
        .collect())
}

async fn feature_image_json(ctx: &AppContext, item: &pages::Model) -> Result<Option<Value>> {
    Ok(item.get_first_media(ctx, FEATURE_IMAGE).await?.map(|media| {
        json!({
            "url": media.url(), "name": media.name, "kb": (media.size + 1023) / 1024,
            "alt": media.custom_property("alt").and_then(|alt| alt.as_str().map(str::to_owned)).unwrap_or_default(),
        })
    }))
}

fn display_title(languages: &[Value], slug: &str) -> String {
    languages
        .iter()
        .find(|language| language["code"] == "en" && language["exists"] == true)
        .or_else(|| languages.iter().find(|language| language["exists"] == true))
        .and_then(|language| language["title"].as_str().map(str::to_owned))
        .unwrap_or_else(|| slug.to_string())
}

/// Read-only view of a page: the details, both languages rendered as visitors see them, and the image.
#[debug_handler]
async fn show(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Path(id): Path<i64>,
) -> Result<Response> {
    if !page.can("pages.view") {
        return page.forbidden(&v);
    }
    let item = find(&ctx, id).await?;
    let languages = language_rows(&ctx, item.id).await?;
    let title = display_title(&languages, &item.slug);
    page.render(
        &v,
        "admin/page_show.html",
        json!({
            "title": title, "page_title": title,
            "item": { "id": item.id, "slug": item.slug, "published": item.is_published, "updated": item.updated_at.format("%Y-%m-%d %H:%M").to_string() },
            "languages": languages, "image": feature_image_json(&ctx, &item).await?, "can_manage": page.can("pages.manage"),
        }),
    )
}

/// Builds and renders the edit screen. `languages` can carry values the person just typed.
async fn render_edit(
    v: &TeraView,
    ctx: &AppContext,
    page: &AdminPage,
    item: &pages::Model,
    languages: Vec<Value>,
    message: Option<Value>,
    status: StatusCode,
) -> Result<Response> {
    let title = display_title(&languages, &item.slug);
    let body = v.render(
        "admin/page_edit.html",
        page.data(json!({
            "title": format!("Edit page: {title}"), "page_title": title,
            "item": { "id": item.id, "slug": item.slug, "published": item.is_published },
            "languages": languages, "image": feature_image_json(ctx, item).await?, "can_manage": page.can("pages.manage"),
            "message": message,
        })),
    )?;
    Ok((status, axum::response::Html(body)).into_response())
}

#[debug_handler]
async fn edit(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Path(id): Path<i64>,
    axum::extract::Query(query): axum::extract::Query<EditQuery>,
) -> Result<Response> {
    if !page.can("pages.view") {
        return page.forbidden(&v);
    }
    let item = find(&ctx, id).await?;
    let languages = language_rows(&ctx, item.id).await?;
    render_edit(
        &v,
        &ctx,
        &page,
        &item,
        languages,
        status_message(&query),
        StatusCode::OK,
    )
    .await
}

/// Saves the text of every language at once. Everything is checked first, so a mistake in one
/// language saves nothing; the form is shown again with what was typed.
#[debug_handler]
async fn save_content(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(id): Path<i64>,
    Form(form): Form<HashMap<String, String>>,
) -> Result<Response> {
    if !page.can("pages.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok(rejected());
    }
    let item = find(&ctx, id).await?;
    let existing = page_translations::Entity::find()
        .filter(page_translations::Column::PageId.eq(item.id))
        .all(&ctx.db)
        .await?;

    struct Cleaned {
        code: &'static str,
        title: String,
        meta: String,
        body: String,
    }
    let mut cleaned = Vec::new();
    let mut typed = Vec::new();
    let mut problem: Option<String> = None;

    for code in SUPPORTED_LOCALES {
        let field = |name: &str| {
            form.get(&format!("{name}_{code}"))
                .map(String::as_str)
                .unwrap_or_default()
        };
        let title = field("title").trim().to_string();
        let meta = field("meta").trim().to_string();
        // Visitors never see script or event handlers: the body is cleaned to a safe set of tags.
        let body = ammonia::clean(field("body").trim());
        let has_row = existing
            .iter()
            .any(|translation| translation.locale == code);
        typed.push(json!({ "code": code, "label": language_name(code), "exists": has_row, "title": title, "meta": meta, "body": field("body").trim() }));

        // A language nobody started on is simply left out.
        if title.is_empty() && body.is_empty() && !has_row {
            continue;
        }
        let name = language_name(code);
        if problem.is_none() {
            problem = if title.is_empty() {
                Some(format!("{name}: the title is required."))
            } else if title.chars().count() > MAX_TITLE_CHARS {
                Some(format!(
                    "{name}: the title is longer than {MAX_TITLE_CHARS} characters."
                ))
            } else if meta.chars().count() > MAX_META_CHARS {
                Some(format!(
                    "{name}: the description is longer than {MAX_META_CHARS} characters."
                ))
            } else if body.is_empty() {
                Some(format!("{name}: the page text is required."))
            } else if body.len() > MAX_BODY_BYTES {
                Some(format!("{name}: the page text is too long."))
            } else {
                None
            };
        }
        cleaned.push(Cleaned {
            code,
            title,
            meta,
            body,
        });
    }

    if let Some(problem) = problem {
        let message = Some(json!({ "kind": "error", "text": problem }));
        return render_edit(
            &v,
            &ctx,
            &page,
            &item,
            typed,
            message,
            StatusCode::UNPROCESSABLE_ENTITY,
        )
        .await;
    }

    for entry in cleaned {
        match existing
            .iter()
            .find(|translation| translation.locale == entry.code)
        {
            Some(row) => {
                let mut active: page_translations::ActiveModel = row.clone().into();
                active.title = Set(entry.title);
                active.meta_description = Set(entry.meta);
                active.body_html = Set(entry.body);
                active.update(&ctx.db).await?;
            }
            None => {
                page_translations::ActiveModel {
                    page_id: Set(item.id),
                    locale: Set(entry.code.to_string()),
                    title: Set(entry.title),
                    meta_description: Set(entry.meta),
                    body_html: Set(entry.body),
                    ..Default::default()
                }
                .insert(&ctx.db)
                .await?;
            }
        }
    }
    let slug = item.slug.clone();
    let mut touched: pages::ActiveModel = item.into();
    touched.updated_at = Set(chrono::Utc::now().fixed_offset());
    touched.update(&ctx.db).await?;
    forget_cache(&ctx, &slug).await;
    Ok(back(id, "saved", "content"))
}

#[debug_handler]
async fn upload_feature_image(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<Response> {
    if !page.can("pages.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok(rejected());
    }
    let item = find(&ctx, id).await?;
    let mut form = read_form(multipart, "image").await;
    let alt: String = form
        .fields
        .remove("alt")
        .map(|alt| alt.trim().chars().take(MAX_ALT_CHARS).collect())
        .unwrap_or_default();
    let file = match form.into_file() {
        Ok(file) => file,
        Err(MediaError::TooLarge { .. }) => return Ok(back(id, "error", "size")),
        Err(_) => return Ok(back(id, "error", "empty")),
    };

    // Single-file collection: this replaces any previous feature image.
    let result = item
        .add_media(&ctx, file)
        .using_name(&format!("Feature image of {}", item.slug))
        .with_custom_properties(json!({ "alt": alt }))
        .to_media_collection(FEATURE_IMAGE)
        .await;
    match result {
        Ok(_) => {
            forget_cache(&ctx, &item.slug).await;
            Ok(back(id, "saved", "image"))
        }
        Err(MediaError::Empty) => Ok(back(id, "error", "empty")),
        Err(MediaError::UnsupportedType) => Ok(back(id, "error", "type")),
        Err(MediaError::TooLarge { .. }) => Ok(back(id, "error", "size")),
        Err(error) => Err(error.into()),
    }
}

#[debug_handler]
async fn remove_feature_image(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(id): Path<i64>,
) -> Result<Response> {
    if !page.can("pages.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok(rejected());
    }
    let item = find(&ctx, id).await?;
    item.clear_media_collection(&ctx, FEATURE_IMAGE).await?;
    forget_cache(&ctx, &item.slug).await;
    Ok(back(id, "saved", "removed"))
}

#[derive(Deserialize)]
struct PublishForm {
    published: String,
}

#[debug_handler]
async fn set_published(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(id): Path<i64>,
    Form(form): Form<PublishForm>,
) -> Result<Response> {
    if !page.can("pages.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok(rejected());
    }
    let item = find(&ctx, id).await?;
    let publish = form.published == "1";
    let slug = item.slug.clone();
    let mut active: pages::ActiveModel = item.into();
    active.is_published = Set(publish);
    active.update(&ctx.db).await?;
    forget_cache(&ctx, &slug).await;
    Ok(back(
        id,
        "saved",
        if publish { "published" } else { "unpublished" },
    ))
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new()
        .add("/admin/pages", get(index))
        .add("/admin/pages/{id}", get(show))
        .add("/admin/pages/{id}/edit", get(edit))
        .add("/admin/pages/{id}/content", post(save_content))
        .add(
            "/admin/pages/{id}/feature-image",
            post(upload_feature_image),
        )
        .add(
            "/admin/pages/{id}/feature-image/remove",
            post(remove_feature_image),
        )
        .add("/admin/pages/{id}/published", post(set_published))]
}
