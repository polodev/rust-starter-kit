use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// One stored file, owned by any model through `model_type` + `model_id` (a polymorphic owner).
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "media_files")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i64,
    pub model_type: String,
    pub model_id: i64,
    #[sea_orm(unique)]
    pub uuid: String,
    pub collection_name: String,
    pub name: String,
    pub file_name: String,
    pub mime_type: String,
    pub disk: String,
    pub size: i64,
    /// JSON text.
    #[sea_orm(column_type = "Text")]
    pub custom_properties: String,
    pub order_column: i32,
    pub is_public: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Where the file is stored on its disk.
    pub fn storage_path(&self) -> String {
        format!("media/{}/{}", self.uuid, self.file_name)
    }

    /// Address served by this module.
    pub fn url(&self) -> String {
        crate::service::url_for(&self.uuid, &self.file_name)
    }

    pub fn custom_property(&self, key: &str) -> Option<serde_json::Value> {
        serde_json::from_str::<serde_json::Value>(&self.custom_properties)
            .ok()?
            .get(key)
            .cloned()
    }
}
