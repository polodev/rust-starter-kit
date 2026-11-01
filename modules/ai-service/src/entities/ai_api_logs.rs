use loco_database::{UnsignedBigInt, UnsignedInt, UnsignedSmallInt};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub const SUCCESS: &str = "success";
pub const FAILED: &str = "failed";
pub const STATUSES: [&str; 2] = [SUCCESS, FAILED];

/// One call to an AI provider, with its timing, tokens, estimated cost and the error if it failed.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ai_api_logs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: UnsignedBigInt,
    pub kind: String,
    pub provider: String,
    pub model: String,
    pub purpose: Option<String>,
    pub status: String,
    pub http_status: Option<UnsignedSmallInt>,
    pub duration_ms: UnsignedInt,
    pub input_tokens: UnsignedInt,
    pub output_tokens: UnsignedInt,
    pub cost: Option<f64>,
    pub image_count: UnsignedSmallInt,
    pub sensitive: bool,
    #[sea_orm(column_type = "Text", nullable)]
    pub prompt: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub response: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub error: Option<String>,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
