use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A waiting job of a Laravel database queue.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "jobs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u64,
    pub queue: String,
    #[sea_orm(column_type = "Text")]
    pub payload: String,
    pub attempts: u8,
    pub reserved_at: Option<u32>,
    pub available_at: u32,
    pub created_at: u32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
