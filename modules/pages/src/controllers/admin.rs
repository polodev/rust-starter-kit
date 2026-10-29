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
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::Deserialize;
use serde_json::json;

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
    let translations = page_translations::Entity::find()
        .filter(page_translations::Column::PageId.eq(item.id))
        .order_by_asc(page_translations::Column::Locale)
        .all(&ctx.db)
        .await?;
    let title = translations
        .iter()
        .find(|t| t.locale == "en")
        .or_else(|| translations.first())
        .map_or_else(|| item.slug.clone(), |t| t.title.clone());

    let image = item.get_first_media(&ctx, FEATURE_IMAGE).await?.map(|media| {
        json!({
            "url": media.url(), "name": media.name, "kb": (media.size + 1023) / 1024,
            "alt": media.custom_property("alt").and_then(|alt| alt.as_str().map(str::to_owned)).unwrap_or_default(),
        })
    });

    let message = match (query.saved.as_deref(), query.error.as_deref()) {
        (Some("image"), _) => Some(("ok", "Feature image saved.")),
        (Some("removed"), _) => Some(("ok", "Feature image removed.")),
        (Some("published"), _) => Some(("ok", "The page is now published.")),
        (Some("unpublished"), _) => Some(("ok", "The page is now hidden from visitors.")),
        (_, Some("empty")) => Some(("error", "Choose an image first.")),
        (_, Some("type")) => Some(("error", "Upload a PNG, JPG, WebP or GIF image.")),
        (_, Some("size")) => Some(("error", "The image is larger than 2 MB.")),
        (_, Some(_)) => Some(("error", "The image could not be saved.")),
        _ => None,
    };
    let translations: Vec<_> = translations
        .iter()
        .map(|t| json!({ "locale": t.locale.to_uppercase(), "title": t.title }))
        .collect();
    page.render(
        &v,
        "admin/page_edit.html",
        json!({
            "title": format!("Edit page: {title}"), "page_title": title, "item": { "id": item.id, "slug": item.slug, "published": item.is_published },
            "translations": translations, "image": image, "can_manage": page.can("pages.manage"),
            "message": message.map(|(kind, text)| json!({ "kind": kind, "text": text })),
        }),
    )
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
        .add("/admin/pages/{id}/edit", get(edit))
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
