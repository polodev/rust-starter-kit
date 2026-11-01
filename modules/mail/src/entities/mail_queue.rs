use loco_database::UnsignedBigInt;
use sea_orm::entity::prelude::*;

/// One email waiting to be sent. `payload` is the JSON of a `QueuedMail`.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "mail_queue")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: UnsignedBigInt,
    pub mail_log_id: UnsignedBigInt,
    #[sea_orm(column_type = "Text")]
    pub payload: String,
    pub attempts: i32,
    /// Not tried before this time (later after each failed attempt).
    pub available_at: DateTimeWithTimeZone,
    /// Set while a worker holds the mail; a worker that died leaves it behind and it is taken over later.
    pub reserved_at: Option<DateTimeWithTimeZone>,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
