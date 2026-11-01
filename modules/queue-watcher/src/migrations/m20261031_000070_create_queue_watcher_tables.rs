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

/// The same tables for PostgreSQL. `jobs` and `failed_jobs` are Laravel's own PostgreSQL layout, so a Laravel
/// PostgreSQL export imports as is.
const CREATE_QUEUE_JOBS_POSTGRES: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS queue_jobs (
  id bigserial PRIMARY KEY,
  job_id varchar(255) NOT NULL,
  name varchar(255),
  job_class varchar(255),
  job_description varchar(255),
  queue varchar(255),
  connection varchar(255),
  started_at timestamptz,
  finished_at timestamptz,
  failed_at timestamptz,
  attempt integer NOT NULL DEFAULT 0,
  max_tries integer,
  timeout integer,
  tags varchar(255),
  progress integer,
  status varchar(255) NOT NULL DEFAULT 'queued',
  exception_message text,
  exception_trace text,
  data text,
  payload text,
  job_parameters text,
  created_at timestamptz,
  updated_at timestamptz
)",
    "CREATE UNIQUE INDEX IF NOT EXISTS queue_jobs_job_id_unique ON queue_jobs (job_id)",
    "CREATE INDEX IF NOT EXISTS queue_jobs_status_created_at_index ON queue_jobs (status, created_at)",
    "CREATE INDEX IF NOT EXISTS queue_jobs_queue_index ON queue_jobs (queue)",
    "CREATE INDEX IF NOT EXISTS queue_jobs_created_at_index ON queue_jobs (created_at)",
    "CREATE INDEX IF NOT EXISTS queue_jobs_job_class_index ON queue_jobs (job_class)",
];

const CREATE_JOBS_POSTGRES: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS jobs (
  id bigserial PRIMARY KEY,
  queue varchar(255) NOT NULL,
  payload text NOT NULL,
  attempts smallint NOT NULL,
  reserved_at integer,
  available_at integer NOT NULL,
  created_at integer NOT NULL
)",
    "CREATE INDEX IF NOT EXISTS jobs_queue_index ON jobs (queue)",
];

const CREATE_FAILED_JOBS_POSTGRES: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS failed_jobs (
  id bigserial PRIMARY KEY,
  uuid varchar(255) NOT NULL,
  connection text NOT NULL,
  queue text NOT NULL,
  payload text NOT NULL,
  exception text NOT NULL,
  failed_at timestamptz NOT NULL DEFAULT now()
)",
    "CREATE UNIQUE INDEX IF NOT EXISTS failed_jobs_uuid_unique ON failed_jobs (uuid)",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        loco_database::run_ddl(m, CREATE_QUEUE_JOBS, CREATE_QUEUE_JOBS_POSTGRES).await?;
        loco_database::run_ddl(m, CREATE_JOBS, CREATE_JOBS_POSTGRES).await?;
        loco_database::run_ddl(m, CREATE_FAILED_JOBS, CREATE_FAILED_JOBS_POSTGRES).await?;
        Ok(())
    }

    /// The `jobs` and `failed_jobs` tables may belong to a Laravel app, so nothing is dropped here.
    async fn down(&self, _m: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
