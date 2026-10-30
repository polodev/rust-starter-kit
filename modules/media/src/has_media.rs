use async_trait::async_trait;
use loco_rs::app::AppContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::{
    collection::CollectionDef,
    entities::media,
    error::MediaResult,
    service::{delete_item, list, FileAdder},
    upload::Upload,
};

/// Implement this for a model to let it own files, like Spatie's `HasMedia`.
pub trait HasMedia: Sync {
    /// Stable name for the model, stored with every file (`"user"`, `"post"`, ...).
    fn media_type(&self) -> String;

    fn media_id(&self) -> i64;

    /// Declares the collections this model uses, like `registerMediaCollections()`.
    fn media_collections(&self) -> Vec<CollectionDef> {
        Vec::new()
    }
}

/// An owner described by name and id, for when you have no model type to implement the trait on.
#[derive(Debug, Clone)]
pub struct Owner {
    model_type: String,
    model_id: i64,
    collections: Vec<CollectionDef>,
}

impl Owner {
    pub fn new(model_type: impl Into<String>, model_id: i64) -> Self {
        Self {
            model_type: model_type.into(),
            model_id,
            collections: Vec::new(),
        }
    }

    pub fn with_collections(mut self, collections: Vec<CollectionDef>) -> Self {
        self.collections = collections;
        self
    }
}

impl HasMedia for Owner {
    fn media_type(&self) -> String {
        self.model_type.clone()
    }

    fn media_id(&self) -> i64 {
        self.model_id
    }

    fn media_collections(&self) -> Vec<CollectionDef> {
        self.collections.clone()
    }
}

/// Everything a model can do with its files; available on every [`HasMedia`] type.
#[async_trait]
pub trait InteractsWithMedia: HasMedia {
    /// Start adding a file: `model.add_media(&ctx, upload).using_name("..").to_media_collection("logo").await`.
    fn add_media<'a>(&self, ctx: &'a AppContext, upload: Upload) -> FileAdder<'a> {
        FileAdder::new(
            ctx,
            self.media_type(),
            self.media_id(),
            self.media_collections(),
            upload,
        )
    }

    async fn get_media(
        &self,
        ctx: &AppContext,
        collection: &str,
    ) -> MediaResult<Vec<media::Model>> {
        list(ctx, &self.media_type(), self.media_id(), collection).await
    }

    async fn get_first_media(
        &self,
        ctx: &AppContext,
        collection: &str,
    ) -> MediaResult<Option<media::Model>> {
        Ok(self.get_media(ctx, collection).await?.into_iter().next())
    }

    async fn get_last_media(
        &self,
        ctx: &AppContext,
        collection: &str,
    ) -> MediaResult<Option<media::Model>> {
        Ok(self.get_media(ctx, collection).await?.pop())
    }

    async fn has_media(&self, ctx: &AppContext, collection: &str) -> MediaResult<bool> {
        Ok(!self.get_media(ctx, collection).await?.is_empty())
    }

    async fn get_first_media_url(
        &self,
        ctx: &AppContext,
        collection: &str,
    ) -> MediaResult<Option<String>> {
        Ok(self
            .get_first_media(ctx, collection)
            .await?
            .map(|media| media.url()))
    }

    /// Deletes every file of the collection; returns how many.
    async fn clear_media_collection(&self, ctx: &AppContext, collection: &str) -> MediaResult<u64> {
        let items = self.get_media(ctx, collection).await?;
        let count = items.len() as u64;
        for item in &items {
            delete_item(ctx, item).await?;
        }
        Ok(count)
    }

    /// Deletes one file, but only if it belongs to this model. Returns whether it was deleted.
    async fn delete_media(&self, ctx: &AppContext, media_id: u64) -> MediaResult<bool> {
        let found = media::Entity::find_by_id(media_id)
            .filter(media::Column::ModelType.eq(self.media_type()))
            .filter(media::Column::ModelId.eq(self.media_id().to_string()))
            .one(&ctx.db)
            .await?;
        match found {
            Some(item) => {
                delete_item(ctx, &item).await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

impl<T: HasMedia + ?Sized> InteractsWithMedia for T {}
