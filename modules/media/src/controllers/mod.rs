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
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::json;

use crate::{
    collection::{CollectionDef, DEFAULT_MAX_BYTES, DOCUMENTS},
    entities::media,
    error::MediaError,
    has_media::{InteractsWithMedia, Owner},
    service::{delete_item, direct_url, prefix},
    upload::Upload,
    LIBRARY_COLLECTION, LIBRARY_OWNER_ID, LIBRARY_OWNER_TYPE,
};

/// A multipart form split into its file and its plain text fields.
#[derive(Debug, Default)]
pub struct ParsedForm {
    pub file: Option<Upload>,
    pub fields: std::collections::HashMap<String, String>,
    /// The file field was present but its data could not be read, which happens when the request
    /// is bigger than the server's body limit. Treat it as "too large", not as "nothing chosen".
    pub file_unreadable: bool,
}

impl ParsedForm {
    /// The file, or the reason there is none.
    pub fn into_file(self) -> Result<Upload, MediaError> {
        match self.file {
            Some(file) => Ok(file),
            None if self.file_unreadable => Err(MediaError::TooLarge {
                max_bytes: crate::DEFAULT_MAX_BYTES,
            }),
            None => Err(MediaError::Empty),
        }
    }
}

/// Reads a whole multipart form: the file in `file_field` (if any) and every other field as text.
pub async fn read_form(mut multipart: Multipart, file_field: &str) -> ParsedForm {
    let mut form = ParsedForm::default();
    while let Ok(Some(field)) = multipart.next_field().await {
        let Some(name) = field.name().map(str::to_owned) else {
            continue;
        };
        if name == file_field {
            let file_name = field.file_name().unwrap_or("upload").to_string();
            match field.bytes().await {
                Ok(bytes) if !bytes.is_empty() => form.file = Some(Upload::new(file_name, bytes)),
                Ok(_) => {}
                Err(_) => form.file_unreadable = true,
            }
        } else if let Ok(text) = field.text().await {
            form.fields.insert(name, text);
        }
    }
    form
}

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

/// Serves a stored file. Public files go to everyone; private files only to their owner and to
/// staff with `media.view`. Anything else looks like a missing file.
#[debug_handler]
async fn serve(
    State(ctx): State<AppContext>,
    OptionalAuth(user): OptionalAuth,
    Path((id, file_name)): Path<(u64, String)>,
) -> Result<Response> {
    let Some(item) = media::Entity::find_by_id(id).one(&ctx.db).await? else {
        return Err(Error::NotFound);
    };
    if item.file_name != file_name {
        return Err(Error::NotFound);
    }
    if !item.is_public() {
        let allowed = user.is_some_and(|user| {
            (item.model_type == "user" && item.owner_id() == Some(user.id))
                || user.access.can("media.view")
        });
        if !allowed {
            return Err(Error::NotFound);
        }
    }
    // A public file on a disk with a public URL (S3, a CDN) is served from there, not through us.
    if item.is_public() {
        if let Some(url) = direct_url(&ctx, &item) {
            return Ok(Redirect::temporary(&url).into_response());
        }
    }
    let bytes: Vec<u8> = ctx
        .storage
        .download(FsPath::new(&item.storage_path(&prefix(&ctx))))
        .await
        .map_err(|_| Error::NotFound)?;
    let bytes = Bytes::from(bytes);

    let cache = if item.is_public() {
        "public, max-age=86400"
    } else {
        "private, max-age=3600"
    };
    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                item.mime_type
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
            ),
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

const PER_PAGE: u64 = 24;

#[derive(Debug, Default, Deserialize)]
struct LibraryQuery {
    q: Option<String>,
    collection: Option<String>,
    kind: Option<String>,
    page: Option<u64>,
    /// `uploaded`, `failed` and `deleted` counters set by the redirects after a change.
    #[serde(flatten)]
    counts: std::collections::HashMap<String, String>,
}

/// Where the owner of a file is edited, when this app has a page for it.
fn owner_link(model_type: &str, model_id: Option<i64>) -> Option<(String, String)> {
    let model_id = model_id?;
    match model_type {
        LIBRARY_OWNER_TYPE => None,
        "page" => Some((
            format!("Page #{model_id}"),
            format!("/admin/pages/{model_id}/edit"),
        )),
        "user" => Some((format!("User #{model_id}"), "/admin/users".to_string())),
        _ => None,
    }
}

/// Every file of every model, newest first. Files are added and removed on the model that owns
/// them (for example a page's feature image); this page is for finding and checking them.
#[debug_handler]
async fn library(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    axum::extract::Query(query): axum::extract::Query<LibraryQuery>,
) -> Result<Response> {
    if !page.can("media.view") {
        return page.forbidden(&v);
    }
    let search = query.q.clone().unwrap_or_default().trim().to_string();
    let collection = query.collection.clone().unwrap_or_default();
    let kind = query.kind.clone().unwrap_or_default();

    let mut select = media::Entity::find();
    if !search.is_empty() {
        select = select.filter(media::Column::Name.contains(search.as_str()));
    }
    if !collection.is_empty() {
        select = select.filter(media::Column::CollectionName.eq(collection.as_str()));
    }
    match kind.as_str() {
        "image" => select = select.filter(media::Column::MimeType.starts_with("image/")),
        "pdf" => select = select.filter(media::Column::MimeType.eq("application/pdf")),
        _ => {}
    }

    let paginator = select
        .order_by_desc(media::Column::Id)
        .paginate(&ctx.db, PER_PAGE);
    let totals = paginator.num_items_and_pages().await?;
    let current = query
        .page
        .unwrap_or(1)
        .clamp(1, totals.number_of_pages.max(1));
    let items = paginator.fetch_page(current - 1).await?;

    let collections: Vec<String> = media::Entity::find()
        .select_only()
        .column(media::Column::CollectionName)
        .distinct()
        .order_by_asc(media::Column::CollectionName)
        .into_tuple::<String>()
        .all(&ctx.db)
        .await?;

    let rows: Vec<_> = items
        .iter()
        .map(|item| {
            let owner = owner_link(&item.model_type, item.owner_id());
            let mime = item.mime_type.clone().unwrap_or_default();
            let is_library = item.model_type == LIBRARY_OWNER_TYPE;
            json!({
                "name": item.name, "file_name": item.file_name, "owner_type": if is_library { "loose file, no record".to_string() } else { item.model_type.clone() }, "url": crate::service::public_url(&ctx, item), "mime": mime, "kb": u64::from(item.size).div_ceil(1024),
                "is_image": mime.starts_with("image/"), "collection": item.collection_name,
                "public": item.is_public(), "added": item.created_at.map(|at| at.format("%Y-%m-%d").to_string()).unwrap_or_default(),
                "is_library": is_library, "id": item.id,
                "owner_label": if is_library { "Media library".to_string() } else { owner.as_ref().map_or_else(|| format!("{} #{}", item.model_type, item.model_id), |(label, _)| label.clone()) },
                "owner_url": owner.map(|(_, url)| url),
            })
        })
        .collect();

    let count = |key: &str| {
        query
            .counts
            .get(key)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0)
    };
    let (uploaded, failed) = (count("uploaded"), count("failed"));
    let reason = match query.counts.get("reason").map(String::as_str) {
        Some("size") => " It is larger than 10 MB.",
        Some("type") => " Only PNG, JPG, WebP, GIF and PDF files are accepted.",
        Some("empty") => " No file was chosen.",
        _ => "",
    };
    let plural = |n: u64| if n == 1 { "" } else { "s" };
    let message = if query.counts.contains_key("deleted") {
        Some(("ok", "File deleted.".to_string()))
    } else if uploaded > 0 && failed == 0 {
        Some((
            "ok",
            format!("{uploaded} file{} uploaded.", plural(uploaded)),
        ))
    } else if uploaded > 0 {
        Some((
            "error",
            format!("{uploaded} uploaded, {failed} failed.{reason}"),
        ))
    } else if failed > 0 {
        Some((
            "error",
            format!(
                "{failed} file{} could not be uploaded.{reason}",
                plural(failed)
            ),
        ))
    } else {
        None
    };
    page.render(
        &v,
        "admin/media.html",
        json!({
            "can_manage": page.can("media.manage"),
            "message": message.map(|(kind, text)| json!({ "kind": kind, "text": text })),
            "title": "Media library", "items": rows, "total": totals.number_of_items,
            "q": search, "collection": collection, "kind": kind, "collections": collections,
            "page": current, "pages": totals.number_of_pages.max(1),
            "prev": if current > 1 { Some(current - 1) } else { None },
            "next": if current < totals.number_of_pages { Some(current + 1) } else { None },
        }),
    )
}

/// Files uploaded directly into the library: pictures and PDFs, public, many per owner.
fn library_owner() -> Owner {
    Owner::new(LIBRARY_OWNER_TYPE, LIBRARY_OWNER_ID).with_collections(vec![
        CollectionDef::multiple(LIBRARY_COLLECTION)
            .accepts_mime_types(DOCUMENTS)
            .max_bytes(DEFAULT_MAX_BYTES)
            .public(),
    ])
}

/// Direct upload: one file per request. The page uploads several files by sending one request each,
/// so every file is checked and reported on its own and stays within the server's body limit.
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
    let outcome = match read_form(multipart, "file").await.into_file() {
        Ok(file) => library_owner()
            .add_media(&ctx, file)
            .to_media_collection(LIBRARY_COLLECTION)
            .await
            .map(|_| ()),
        Err(error) => Err(error),
    };
    match outcome {
        Ok(()) => Ok(Redirect::to("/admin/media?uploaded=1").into_response()),
        Err(error) if error.is_user_error() => {
            let reason = match error {
                MediaError::Empty => "empty",
                MediaError::TooLarge { .. } => "size",
                _ => "type",
            };
            Ok(Redirect::to(&format!("/admin/media?failed=1&reason={reason}")).into_response())
        }
        Err(error) => Err(error.into()),
    }
}

/// Deletes a loose library file. Files that belong to a record are removed on that record instead.
#[debug_handler]
async fn delete(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(id): Path<u64>,
) -> Result<Response> {
    if !page.can("media.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    let item = media::Entity::find_by_id(id)
        .filter(media::Column::ModelType.eq(LIBRARY_OWNER_TYPE))
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    delete_item(&ctx, &item).await?;
    Ok(Redirect::to("/admin/media?deleted=1").into_response())
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new()
        .add("/media/{id}/{file}", get(serve))
        .add("/admin/media", get(library))
        .add("/admin/media/upload", post(upload))
        .add("/admin/media/{id}/delete", post(delete))]
}
