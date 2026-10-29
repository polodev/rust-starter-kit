use sea_orm::entity::prelude::*;

pub const STATUS_NEW: &str = "new";
pub const STATUS_REPLIED: &str = "replied";

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "contacts")]
pub struct Model {
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub email: String,
    pub subject: Option<String>,
    pub message: String,
    /// `new` until someone answers, then `replied`.
    pub status: String,
    pub replied_at: Option<DateTimeWithTimeZone>,
    pub locale: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
