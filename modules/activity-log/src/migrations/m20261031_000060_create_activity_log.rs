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

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.get_connection()
            .execute_unprepared(CREATE_ACTIVITY_LOG)
            .await?;
        Ok(())
    }

    /// The table may be shared with a Laravel app, so it is never dropped here.
    async fn down(&self, _m: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
