use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "page_translations")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i64,
    pub page_id: i64,
    /// Short language code: `en` or `bn`.
    pub locale: String,
    pub title: String,
    pub meta_description: String,
    /// Trusted HTML written by staff; rendered with `| safe`.
    #[sea_orm(column_type = "Text")]
    pub body_html: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::pages::Entity",
        from = "Column::PageId",
        to = "super::pages::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Pages,
}

impl Related<super::pages::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Pages.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
