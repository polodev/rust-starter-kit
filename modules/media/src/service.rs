use std::path::Path;

use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    collection::CollectionDef,
    entities::media,
    error::{MediaError, MediaResult},
    upload::{extension_for, sanitize_stem, Upload},
};

/// Address of a stored file as served by this module: `/media/{id}/{file_name}`.
pub fn url_for(id: u64, file_name: &str) -> String {
    format!("/media/{id}/{file_name}")
}

/// Settings under `settings.media` in the config file (all optional):
///
/// ```yaml
/// settings:
///   media:
///     disk: s3-media        # name written to new rows (Spatie's `disk` column); default "default"
///     prefix: ""            # key prefix, like Spatie's `media-library.prefix`
///     disk_urls:            # public base URL per disk; public files are redirected there
///       s3-media: https://cdn.example.com
/// ```
fn setting(ctx: &AppContext, path: &[&str]) -> Option<Value> {
    let mut current = ctx.config.settings.as_ref()?.get("media")?;
    for key in path {
        current = current.get(key)?;
    }
    Some(current.clone())
}

fn setting_text(ctx: &AppContext, path: &[&str]) -> Option<String> {
    setting(ctx, path)?
        .as_str()
        .map(|text| text.trim_matches('/').to_string())
        .filter(|text| !text.is_empty())
}

pub fn prefix(ctx: &AppContext) -> String {
    setting_text(ctx, &["prefix"]).unwrap_or_default()
}

pub fn disk_name(ctx: &AppContext) -> String {
    setting_text(ctx, &["disk"]).unwrap_or_else(|| "default".to_string())
}

/// The file's own address on its disk (for example a CDN), when that disk has a public URL set.
pub fn direct_url(ctx: &AppContext, item: &media::Model) -> Option<String> {
    let base = setting(ctx, &["disk_urls", item.disk.as_str()])?;
    let base = base.as_str()?.trim_end_matches('/');
    let path = item.storage_path(&prefix(ctx));
    (!base.is_empty()).then(|| format!("{base}/{path}"))
}

/// Builder for adding one file to a model, like Spatie's `FileAdder`.
pub struct FileAdder<'a> {
    ctx: &'a AppContext,
    owner_type: String,
    owner_id: i64,
    collections: Vec<CollectionDef>,
    upload: Upload,
    name: Option<String>,
    file_name: Option<String>,
    custom_properties: Value,
    order: Option<u32>,
}

impl<'a> FileAdder<'a> {
    pub fn new(
        ctx: &'a AppContext,
        owner_type: String,
        owner_id: i64,
        collections: Vec<CollectionDef>,
        upload: Upload,
    ) -> Self {
        Self {
            ctx,
            owner_type,
            owner_id,
            collections,
            upload,
            name: None,
            file_name: None,
            custom_properties: json!({}),
            order: None,
        }
    }

    /// Display name (default: the uploaded file name without its extension).
    pub fn using_name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// Stored file name (default: the sanitised upload name). The extension always follows the content.
    pub fn using_file_name(mut self, file_name: &str) -> Self {
        self.file_name = Some(file_name.to_string());
        self
    }

    pub fn with_custom_properties(mut self, properties: Value) -> Self {
        self.custom_properties = properties;
        self
    }

    pub fn set_order(mut self, order: u32) -> Self {
        self.order = Some(order);
        self
    }

    /// Validates, stores and records the file, then trims the collection to its limit.
    pub async fn to_media_collection(self, collection: &str) -> MediaResult<media::Model> {
        let definition = self
            .collections
            .iter()
            .find(|def| def.name == collection)
            .copied()
            .unwrap_or_else(|| CollectionDef::multiple(""));

        if self.upload.bytes.is_empty() {
            return Err(MediaError::Empty);
        }
        if self.upload.bytes.len() > definition.max_bytes {
            return Err(MediaError::TooLarge {
                max_bytes: definition.max_bytes,
            });
        }
        let mime = self
            .upload
            .sniff_mime()
            .ok_or(MediaError::UnsupportedType)?;
        if !definition.accepts(mime) {
            return Err(MediaError::UnsupportedType);
        }

        let db = &self.ctx.db;
        let uuid = Uuid::new_v4().to_string();
        let stem = sanitize_stem(self.file_name.as_deref().unwrap_or(&self.upload.file_name));
        let file_name = format!("{stem}.{}", extension_for(mime));
        let name = self.name.unwrap_or_else(|| {
            let original = self
                .upload
                .file_name
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or("file");
            original
                .rsplit_once('.')
                .map_or(original, |(name, _)| name)
                .chars()
                .take(255)
                .collect()
        });
        let order = match self.order {
            Some(order) => order,
            None => media::Entity::find()
                .filter(media::Column::ModelType.eq(self.owner_type.as_str()))
                .filter(media::Column::ModelId.eq(self.owner_id.to_string()))
                .filter(media::Column::CollectionName.eq(collection))
                .order_by_desc(media::Column::OrderColumn)
                .one(db)
                .await?
                .map_or(1, |last| last.order_column.unwrap_or(0) + 1),
        };

        // Like Spatie, the row comes first: the file's folder is the row's id.
        let mut properties = match self.custom_properties {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        if definition.public {
            properties.insert("public".to_string(), json!(true));
        }
        let now = chrono::Utc::now().fixed_offset();
        let media = media::ActiveModel {
            model_type: Set(self.owner_type.clone()),
            model_id: Set(self.owner_id.to_string()),
            uuid: Set(Some(uuid)),
            collection_name: Set(collection.to_string()),
            name: Set(name),
            file_name: Set(file_name),
            mime_type: Set(Some(mime.to_string())),
            disk: Set(disk_name(self.ctx)),
            conversions_disk: Set(None),
            size: Set(u32::try_from(self.upload.bytes.len()).unwrap_or(u32::MAX)),
            manipulations: Set("[]".to_string()),
            custom_properties: Set(Value::Object(properties).to_string()),
            generated_conversions: Set(Some("[]".to_string())),
            responsive_images: Set("[]".to_string()),
            order_column: Set(Some(order)),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
            ..Default::default()
        }
        .insert(db)
        .await?;

        let path = media.storage_path(&prefix(self.ctx));
        if let Err(error) = self
            .ctx
            .storage
            .upload(Path::new(&path), &self.upload.bytes)
            .await
        {
            let _ = media::Entity::delete_by_id(media.id).exec(db).await;
            return Err(MediaError::Storage(error.to_string()));
        }

        if let Some(limit) = definition.keep_limit() {
            let items = list(self.ctx, &self.owner_type, self.owner_id, collection).await?;
            let excess = items.len().saturating_sub(limit);
            // `list` is oldest first, so the oldest files go; the file just added is the newest.
            for old in items.into_iter().take(excess) {
                delete_item(self.ctx, &old).await?;
            }
        }
        Ok(media)
    }
}

/// Files of one owner's collection, oldest first (by `order_column`, then id).
pub async fn list(
    ctx: &AppContext,
    owner_type: &str,
    owner_id: i64,
    collection: &str,
) -> MediaResult<Vec<media::Model>> {
    Ok(media::Entity::find()
        .filter(media::Column::ModelType.eq(owner_type))
        .filter(media::Column::ModelId.eq(owner_id.to_string()))
        .filter(media::Column::CollectionName.eq(collection))
        .order_by_asc(media::Column::OrderColumn)
        .order_by_asc(media::Column::Id)
        .all(&ctx.db)
        .await?)
}

/// Removes the stored file and its row.
pub async fn delete_item(ctx: &AppContext, item: &media::Model) -> MediaResult<()> {
    // A file that is already gone from the disk must not block removing the row.
    let _ = ctx
        .storage
        .delete(Path::new(&item.storage_path(&prefix(ctx))))
        .await;
    media::Entity::delete_by_id(item.id).exec(&ctx.db).await?;
    Ok(())
}
