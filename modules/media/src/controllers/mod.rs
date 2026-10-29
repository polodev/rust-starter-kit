use std::path::Path as FsPath;

use axum::{
    body::Body,
    extract::{Multipart, Path},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use loco_rs::prelude::*;
use module_admin_layout::AdminPage;
use module_auth::OptionalAuth;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::json;

use crate::{entities::media_files, error::MediaError, upload::Upload};

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

const PER_PAGE: u64 = 24;

#[derive(Debug, Default, Deserialize)]
struct LibraryQuery {
    q: Option<String>,
    collection: Option<String>,
    kind: Option<String>,
    page: Option<u64>,
}

/// Where the owner of a file is edited, when this app has a page for it.
fn owner_link(model_type: &str, model_id: i64) -> Option<(String, String)> {
    match model_type {
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

    let mut select = media_files::Entity::find();
    if !search.is_empty() {
        select = select.filter(media_files::Column::Name.contains(search.as_str()));
    }
    if !collection.is_empty() {
        select = select.filter(media_files::Column::CollectionName.eq(collection.as_str()));
    }
    match kind.as_str() {
        "image" => select = select.filter(media_files::Column::MimeType.starts_with("image/")),
        "pdf" => select = select.filter(media_files::Column::MimeType.eq("application/pdf")),
        _ => {}
    }

    let paginator = select
        .order_by_desc(media_files::Column::Id)
        .paginate(&ctx.db, PER_PAGE);
    let totals = paginator.num_items_and_pages().await?;
    let current = query
        .page
        .unwrap_or(1)
        .clamp(1, totals.number_of_pages.max(1));
    let items = paginator.fetch_page(current - 1).await?;

    let collections: Vec<String> = media_files::Entity::find()
        .select_only()
        .column(media_files::Column::CollectionName)
        .distinct()
        .order_by_asc(media_files::Column::CollectionName)
        .into_tuple::<String>()
        .all(&ctx.db)
        .await?;

    let rows: Vec<_> = items
        .iter()
        .map(|item| {
            let owner = owner_link(&item.model_type, item.model_id);
            json!({
                "name": item.name, "file_name": item.file_name, "owner_type": item.model_type, "url": item.url(), "mime": item.mime_type, "kb": (item.size + 1023) / 1024,
                "is_image": item.mime_type.starts_with("image/"), "collection": item.collection_name,
                "public": item.is_public, "added": item.created_at.format("%Y-%m-%d").to_string(),
                "owner_label": owner.as_ref().map_or_else(|| format!("{} #{}", item.model_type, item.model_id), |(label, _)| label.clone()),
                "owner_url": owner.map(|(_, url)| url),
            })
        })
        .collect();

    page.render(
        &v,
        "admin/media.html",
        json!({
            "title": "Media library", "items": rows, "total": totals.number_of_items,
            "q": search, "collection": collection, "kind": kind, "collections": collections,
            "page": current, "pages": totals.number_of_pages.max(1),
            "prev": if current > 1 { Some(current - 1) } else { None },
            "next": if current < totals.number_of_pages { Some(current + 1) } else { None },
        }),
    )
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new()
        .add("/media/{uuid}/{file}", get(serve))
        .add("/admin/media", get(library))]
}
