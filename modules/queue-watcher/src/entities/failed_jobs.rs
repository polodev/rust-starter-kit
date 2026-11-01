use loco_database::UnsignedBigInt;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// A job that a Laravel app gave up on.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "failed_jobs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: UnsignedBigInt,
    #[sea_orm(unique)]
    pub uuid: String,
    #[sea_orm(column_type = "Text")]
    pub connection: String,
    #[sea_orm(column_type = "Text")]
    pub queue: String,
    #[sea_orm(column_type = "Text")]
    pub payload: String,
    #[sea_orm(column_type = "Text")]
    pub exception: String,
    pub failed_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
