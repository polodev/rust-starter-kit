use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Spatie laravel-activitylog's `activity_log` table. The subject and causer ids are text, so models with
/// numeric and uuid keys both fit. `IF NOT EXISTS` makes this a no-op when a Laravel app already made it.
const CREATE_ACTIVITY_LOG: &str = "
CREATE TABLE IF NOT EXISTS `activity_log` (
  `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
  `log_name` varchar(191) DEFAULT NULL,
  `description` text NOT NULL,
  `subject_type` varchar(191) DEFAULT NULL,
  `event` varchar(150) DEFAULT NULL,
  `subject_id` varchar(50) DEFAULT NULL,
  `causer_type` varchar(191) DEFAULT NULL,
  `causer_id` varchar(50) DEFAULT NULL,
  `properties` longtext DEFAULT NULL,
  `batch_uuid` char(36) DEFAULT NULL,
  `created_at` timestamp NULL DEFAULT NULL,
  `updated_at` timestamp NULL DEFAULT NULL,
  PRIMARY KEY (`id`),
  KEY `subject` (`subject_type`,`subject_id`),
  KEY `causer` (`causer_type`,`causer_id`),
  KEY `activity_log_log_name_index` (`log_name`),
  KEY `activity_log_created_at_index` (`created_at`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

/// The same table for PostgreSQL (Spatie's PostgreSQL layout; index names are unique per schema there).
const CREATE_ACTIVITY_LOG_POSTGRES: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS activity_log (
  id bigserial PRIMARY KEY,
  log_name varchar(191),
  description text NOT NULL,
  subject_type varchar(191),
  event varchar(150),
  subject_id varchar(50),
  causer_type varchar(191),
  causer_id varchar(50),
  properties text,
  batch_uuid varchar(36),
  created_at timestamptz,
  updated_at timestamptz
)",
    "CREATE INDEX IF NOT EXISTS activity_log_subject_index ON activity_log (subject_type, subject_id)",
    "CREATE INDEX IF NOT EXISTS activity_log_causer_index ON activity_log (causer_type, causer_id)",
    "CREATE INDEX IF NOT EXISTS activity_log_log_name_index ON activity_log (log_name)",
    "CREATE INDEX IF NOT EXISTS activity_log_created_at_index ON activity_log (created_at)",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        loco_database::run_ddl(m, CREATE_ACTIVITY_LOG, CREATE_ACTIVITY_LOG_POSTGRES).await
    }

    /// The table may be shared with a Laravel app, so it is never dropped here.
    async fn down(&self, _m: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
