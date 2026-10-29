use std::path::Path as FsPath;

use axum::{
    body::Body,
    extract::{Multipart, Path},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use bytes::Bytes;
use loco_rs::prelude::*;
use module_admin_layout::AdminPage;
use module_auth::{same_origin, OptionalAuth};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::json;

use crate::{
    collection::{CollectionDef, DEFAULT_MAX_BYTES, DOCUMENTS},
    entities::media_files,
    has_media::{InteractsWithMedia, Owner},
    service::delete_item,
    upload::Upload,
    MediaError, LIBRARY_COLLECTION, LIBRARY_OWNER_TYPE,
};

/// Reads the first multipart field called `field_name` as an upload, if present.
pub async fn read_upload(mut multipart: Multipart, field_name: &str) -> Option<Upload> {
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some(field_name) {
            let file_name = field.file_name().unwrap_or("upload").to_string();
            let bytes = field.bytes().await.ok()?;
            return Some(Upload::new(file_name, bytes));
        }
    }
    None
}

fn library_owner() -> Owner {
    Owner::new(LIBRARY_OWNER_TYPE, 0).with_collections(vec![CollectionDef::multiple(
        LIBRARY_COLLECTION,
    )
    .accepts_mime_types(DOCUMENTS)
    .max_bytes(DEFAULT_MAX_BYTES)
    .public()])
}

/// Serves a stored file. Public files go to everyone; private files only to their owner and to
/// staff with `media.view`. Anything else looks like a missing file.
#[debug_handler]
async fn serve(
    State(ctx): State<AppContext>,
    OptionalAuth(user): OptionalAuth,
    Path((uuid, file_name)): Path<(String, String)>,
) -> Result<Response> {
    let Some(item) = media_files::Entity::find()
        .filter(media_files::Column::Uuid.eq(uuid.as_str()))
        .one(&ctx.db)
        .await?
    else {
        return Err(Error::NotFound);
    };
    if item.file_name != file_name {
        return Err(Error::NotFound);
    }
    if !item.is_public {
        let allowed = user.is_some_and(|user| {
            (item.model_type == "user" && item.model_id == user.id) || user.access.can("media.view")
        });
        if !allowed {
            return Err(Error::NotFound);
        }
    }
    let bytes: Vec<u8> = ctx
        .storage
        .download(FsPath::new(&item.storage_path()))
        .await
        .map_err(|_| Error::NotFound)?;
    let bytes = Bytes::from(bytes);

    let cache = if item.is_public {
        "public, max-age=86400"
    } else {
        "private, max-age=3600"
    };
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, item.mime_type.clone()),
            (header::CONTENT_LENGTH, bytes.len().to_string()),
            (header::CACHE_CONTROL, cache.to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; sandbox".to_string(),
            ),
        ],
        Body::from(bytes),
    )
        .into_response())
}

fn error_code(error: &MediaError) -> &'static str {
    match error {
        MediaError::Empty => "empty",
        MediaError::TooLarge { .. } => "size",
        MediaError::UnsupportedType => "type",
        _ => "failed",
    }
}

#[debug_handler]
async fn library(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Response> {
    if !page.can("media.view") {
        return page.forbidden(&v);
    }
    let mut items = library_owner().get_media(&ctx, LIBRARY_COLLECTION).await?;
    items.reverse();
    let rows: Vec<_> = items
        .iter()
        .map(|item| {
            json!({
                "id": item.id, "name": item.name, "url": item.url(), "mime": item.mime_type,
                "kb": (item.size + 1023) / 1024, "is_image": item.mime_type.starts_with("image/"),
                "added": item.created_at.format("%Y-%m-%d").to_string(),
            })
        })
        .collect();
    let message = match (query.get("saved"), query.get("error").map(String::as_str)) {
        (Some(_), _) => Some(("ok", "File uploaded.")),
        (_, Some("empty")) => Some(("error", "Choose a file first.")),
        (_, Some("size")) => Some(("error", "That file is larger than 2 MB.")),
        (_, Some("type")) => Some((
            "error",
            "Only PNG, JPG, WebP, GIF and PDF files are accepted.",
        )),
        (_, Some(_)) => Some(("error", "The file could not be saved.")),
        _ => None,
    };
    page.render(
        &v,
        "admin/media.html",
        json!({
            "title": "Media library", "items": rows, "can_manage": page.can("media.manage"),
            "message": message.map(|(kind, text)| json!({ "kind": kind, "text": text })),
        }),
    )
}

#[debug_handler]
async fn upload(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    multipart: Multipart,
) -> Result<Response> {
    if !page.can("media.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    let Some(file) = read_upload(multipart, "file").await else {
        return Ok(Redirect::to("/admin/media?error=empty").into_response());
    };
    match library_owner()
        .add_media(&ctx, file)
        .to_media_collection(LIBRARY_COLLECTION)
        .await
    {
        Ok(_) => Ok(Redirect::to("/admin/media?saved=1").into_response()),
        Err(error) if error.is_user_error() => {
            Ok(Redirect::to(&format!("/admin/media?error={}", error_code(&error))).into_response())
        }
        Err(error) => Err(error.into()),
    }
}

#[debug_handler]
async fn delete(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(id): Path<i64>,
) -> Result<Response> {
    if !page.can("media.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    let item = media_files::Entity::find_by_id(id)
        .filter(media_files::Column::ModelType.eq(LIBRARY_OWNER_TYPE))
        .order_by_asc(media_files::Column::Id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    delete_item(&ctx, &item).await?;
    Ok(Redirect::to("/admin/media").into_response())
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new()
        .add("/media/{uuid}/{file}", get(serve))
        .add("/admin/media", get(library))
        .add("/admin/media/upload", post(upload))
        .add("/admin/media/{id}/delete", post(delete))]
}
