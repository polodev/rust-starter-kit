use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub const QUEUED: &str = "queued";
pub const SENT: &str = "sent";
pub const FAILED: &str = "failed";

/// One email the app tried to send: who, what, how it went.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "mail_log")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: u64,
    pub recipients: String,
    pub from_address: String,
    pub subject: String,
    pub driver: String,
    /// `queued`, `sent` or `failed`.
    pub status: String,
    pub attempts: i32,
    #[sea_orm(column_type = "Text", nullable)]
    pub error: Option<String>,
    /// The provider's answer for an accepted message (for SES, its message id).
    pub provider_id: Option<String>,
    /// File names of the attachments, comma separated.
    pub attachments: Option<String>,
    pub sent_at: Option<DateTimeWithTimeZone>,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
