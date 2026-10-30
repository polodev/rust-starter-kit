use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// One stored file. The table is Spatie laravel-medialibrary's `media` table, column for column
/// (including its unsigned integer types), so a Laravel app and this app can share one database.
/// A file is owned by any model through `model_type` + `model_id`.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "media")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u64,
    /// A Laravel morph alias (`enrollment`) or class name (`Modules\Foo\Models\Foo`), or this
    /// app's own name for the model (`user`, `page`).
    pub model_type: String,
    /// Stored as text, like Laravel does when its models use uuid or ulid keys.
    pub model_id: String,
    pub uuid: Option<String>,
    pub collection_name: String,
    pub name: String,
    pub file_name: String,
    pub mime_type: Option<String>,
    pub disk: String,
    pub conversions_disk: Option<String>,
    pub size: u32,
    /// JSON text.
    #[sea_orm(column_type = "Text")]
    pub manipulations: String,
    /// JSON text.
    #[sea_orm(column_type = "Text")]
    pub custom_properties: String,
    /// JSON text.
    #[sea_orm(column_type = "Text", nullable)]
    pub generated_conversions: Option<String>,
    /// JSON text.
    #[sea_orm(column_type = "Text")]
    pub responsive_images: String,
    pub order_column: Option<u32>,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// The owner's id when it is a number (this app's models); Laravel models with uuid keys give `None`.
    pub fn owner_id(&self) -> Option<i64> {
        self.model_id.parse().ok()
    }

    /// Where the file is stored on its disk. Rows moved over from the older `media_files` table
    /// remember their original location in the `legacy_path` property.
    pub fn storage_path(&self, prefix: &str) -> String {
        if let Some(path) = self
            .custom_property("legacy_path")
            .and_then(|value| value.as_str().map(str::to_owned))
        {
            return path;
        }
        let folder = if prefix.is_empty() {
            self.id.to_string()
        } else {
            format!("{prefix}/{}", self.id)
        };
        format!("{folder}/{}", self.file_name)
    }

    /// Address served by this module (public files redirect to the disk's own URL when one is set).
    pub fn url(&self) -> String {
        crate::service::url_for(self.id, &self.file_name)
    }

    /// Public files are open to everyone. A file is private unless its `public` property says
    /// otherwise, so files imported from Laravel are private until a collection opts in.
    pub fn is_public(&self) -> bool {
        matches!(
            self.custom_property("public"),
            Some(serde_json::Value::Bool(true))
        ) || matches!(self.custom_property("public"), Some(serde_json::Value::Number(n)) if n.as_i64() == Some(1))
    }

    pub fn custom_property(&self, key: &str) -> Option<serde_json::Value> {
        serde_json::from_str::<serde_json::Value>(&self.custom_properties)
            .ok()?
            .get(key)
            .cloned()
    }
}
