/// Spatie's `activity_log` table, column for column. The id columns of the subject and the causer are
/// text, so models with numeric and uuid keys can both be recorded.
pub mod activity_log {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "activity_log")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: u64,
        pub log_name: Option<String>,
        #[sea_orm(column_type = "Text")]
        pub description: String,
        pub subject_type: Option<String>,
        pub event: Option<String>,
        pub subject_id: Option<String>,
        pub causer_type: Option<String>,
        pub causer_id: Option<String>,
        /// JSON text: `{"attributes": {...}, "old": {...}, ...extra}`.
        #[sea_orm(column_type = "Text", nullable)]
        pub properties: Option<String>,
        pub batch_uuid: Option<String>,
        pub created_at: Option<DateTimeWithTimeZone>,
        pub updated_at: Option<DateTimeWithTimeZone>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    impl Model {
        /// The properties as JSON (`Null` when empty or unreadable).
        pub fn properties_json(&self) -> serde_json::Value {
            self.properties
                .as_deref()
                .and_then(|text| serde_json::from_str(text).ok())
                .unwrap_or(serde_json::Value::Null)
        }
    }
}
