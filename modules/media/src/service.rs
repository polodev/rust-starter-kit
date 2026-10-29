use std::path::Path;

use loco_rs::app::AppContext;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    collection::CollectionDef,
    entities::media_files,
    error::{MediaError, MediaResult},
    upload::{extension_for, sanitize_stem, Upload},
};

/// Address of a stored file as served by this module.
pub fn url_for(uuid: &str, file_name: &str) -> String {
    format!("/media/{uuid}/{file_name}")
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
    order: Option<i32>,
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

    pub fn set_order(mut self, order: i32) -> Self {
        self.order = Some(order);
        self
    }

    /// Validates, stores and records the file, then trims the collection to its limit.
    pub async fn to_media_collection(self, collection: &str) -> MediaResult<media_files::Model> {
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
            None => media_files::Entity::find()
                .filter(media_files::Column::ModelType.eq(self.owner_type.as_str()))
                .filter(media_files::Column::ModelId.eq(self.owner_id))
                .filter(media_files::Column::CollectionName.eq(collection))
                .order_by_desc(media_files::Column::OrderColumn)
                .one(db)
                .await?
                .map_or(1, |last| last.order_column + 1),
        };

        let path = format!("media/{uuid}/{file_name}");
        self.ctx
            .storage
            .upload(Path::new(&path), &self.upload.bytes)
            .await
            .map_err(|error| MediaError::Storage(error.to_string()))?;

        let inserted = media_files::ActiveModel {
            model_type: Set(self.owner_type.clone()),
            model_id: Set(self.owner_id),
            uuid: Set(uuid),
            collection_name: Set(collection.to_string()),
            name: Set(name),
            file_name: Set(file_name),
            mime_type: Set(mime.to_string()),
            disk: Set("default".to_string()),
            size: Set(i64::try_from(self.upload.bytes.len()).unwrap_or(i64::MAX)),
            custom_properties: Set(self.custom_properties.to_string()),
            order_column: Set(order),
            is_public: Set(definition.public),
            ..Default::default()
        }
        .insert(db)
        .await;

        let media = match inserted {
            Ok(media) => media,
            Err(error) => {
                let _ = self.ctx.storage.delete(Path::new(&path)).await;
                return Err(error.into());
            }
        };

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
) -> MediaResult<Vec<media_files::Model>> {
    Ok(media_files::Entity::find()
        .filter(media_files::Column::ModelType.eq(owner_type))
        .filter(media_files::Column::ModelId.eq(owner_id))
        .filter(media_files::Column::CollectionName.eq(collection))
        .order_by_asc(media_files::Column::OrderColumn)
        .order_by_asc(media_files::Column::Id)
        .all(&ctx.db)
        .await?)
}

/// Removes the stored file and its row.
pub async fn delete_item(ctx: &AppContext, item: &media_files::Model) -> MediaResult<()> {
    // A file that is already gone from the disk must not block removing the row.
    let _ = ctx.storage.delete(Path::new(&item.storage_path())).await;
    media_files::Entity::delete_by_id(item.id)
        .exec(&ctx.db)
        .await?;
    Ok(())
}
