use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Tracked job history: one row per run of a background job, with its status and any error.
const CREATE_QUEUE_JOBS: &str = "
CREATE TABLE IF NOT EXISTS `queue_jobs` (
  `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
  `job_id` varchar(255) NOT NULL,
  `name` varchar(255) DEFAULT NULL,
  `job_class` varchar(255) DEFAULT NULL,
  `job_description` varchar(255) DEFAULT NULL,
  `queue` varchar(255) DEFAULT NULL,
  `connection` varchar(255) DEFAULT NULL,
  `started_at` timestamp NULL DEFAULT NULL,
  `finished_at` timestamp NULL DEFAULT NULL,
  `failed_at` timestamp NULL DEFAULT NULL,
  `attempt` int(11) NOT NULL DEFAULT 0,
  `max_tries` int(11) DEFAULT NULL,
  `timeout` int(11) DEFAULT NULL,
  `tags` varchar(255) DEFAULT NULL,
  `progress` int(11) DEFAULT NULL,
  `status` varchar(255) NOT NULL DEFAULT 'queued',
  `exception_message` text DEFAULT NULL,
  `exception_trace` longtext DEFAULT NULL,
  `data` longtext DEFAULT NULL,
  `payload` longtext DEFAULT NULL,
  `job_parameters` longtext DEFAULT NULL,
  `created_at` timestamp NULL DEFAULT NULL,
  `updated_at` timestamp NULL DEFAULT NULL,
  PRIMARY KEY (`id`),
  UNIQUE KEY `queue_jobs_job_id_unique` (`job_id`),
  KEY `queue_jobs_status_created_at_index` (`status`,`created_at`),
  KEY `queue_jobs_queue_index` (`queue`),
  KEY `queue_jobs_created_at_index` (`created_at`),
  KEY `queue_jobs_job_class_index` (`job_class`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

/// A Laravel database queue's waiting jobs (the `jobs` table). A Laravel app sharing the database writes it.
const CREATE_JOBS: &str = "
CREATE TABLE IF NOT EXISTS `jobs` (
  `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
  `queue` varchar(255) NOT NULL,
  `payload` longtext NOT NULL,
  `attempts` tinyint(3) unsigned NOT NULL,
  `reserved_at` int(10) unsigned DEFAULT NULL,
  `available_at` int(10) unsigned NOT NULL,
  `created_at` int(10) unsigned NOT NULL,
  PRIMARY KEY (`id`),
  KEY `jobs_queue_index` (`queue`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

/// A Laravel app's failed jobs (the `failed_jobs` table).
const CREATE_FAILED_JOBS: &str = "
CREATE TABLE IF NOT EXISTS `failed_jobs` (
  `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
  `uuid` varchar(255) NOT NULL,
  `connection` text NOT NULL,
  `queue` text NOT NULL,
  `payload` longtext NOT NULL,
  `exception` longtext NOT NULL,
  `failed_at` timestamp NOT NULL DEFAULT current_timestamp(),
  PRIMARY KEY (`id`),
  UNIQUE KEY `failed_jobs_uuid_unique` (`uuid`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        for sql in [CREATE_QUEUE_JOBS, CREATE_JOBS, CREATE_FAILED_JOBS] {
            db.execute_unprepared(sql).await?;
        }
        Ok(())
    }

    /// The `jobs` and `failed_jobs` tables may belong to a Laravel app, so nothing is dropped here.
    async fn down(&self, _m: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
