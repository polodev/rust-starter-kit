use loco_database::UnsignedBigInt;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub const QUEUED: &str = "queued";
pub const RUNNING: &str = "running";
pub const FINISHED: &str = "finished";
pub const FAILED: &str = "failed";
pub const STALE: &str = "stale";
pub const STATUSES: [&str; 5] = [QUEUED, RUNNING, FINISHED, FAILED, STALE];

/// One run of a background job: when it started and ended, its status, and the error if it failed.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "queue_jobs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: UnsignedBigInt,
    #[sea_orm(unique)]
    pub job_id: String,
    pub name: Option<String>,
    pub job_class: Option<String>,
    pub job_description: Option<String>,
    pub queue: Option<String>,
    pub connection: Option<String>,
    pub started_at: Option<DateTimeWithTimeZone>,
    pub finished_at: Option<DateTimeWithTimeZone>,
    pub failed_at: Option<DateTimeWithTimeZone>,
    pub attempt: i32,
    pub max_tries: Option<i32>,
    pub timeout: Option<i32>,
    pub tags: Option<String>,
    pub progress: Option<i32>,
    pub status: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub exception_message: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub exception_trace: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub data: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub payload: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub job_parameters: Option<String>,
    pub created_at: Option<DateTimeWithTimeZone>,
    pub updated_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Seconds between start and end (or failure), when both are known.
    pub fn seconds(&self) -> Option<f64> {
        let start = self.started_at?;
        let end = self.finished_at.or(self.failed_at)?;
        Some((end - start).num_milliseconds() as f64 / 1000.0)
    }
}
