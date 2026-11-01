use loco_database::{UnsignedBigInt, UnsignedInt, UnsignedTinyInt};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A waiting job of a Laravel database queue.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "jobs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: UnsignedBigInt,
    pub queue: String,
    #[sea_orm(column_type = "Text")]
    pub payload: String,
    pub attempts: UnsignedTinyInt,
    pub reserved_at: Option<UnsignedInt>,
    pub available_at: UnsignedInt,
    pub created_at: UnsignedInt,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
