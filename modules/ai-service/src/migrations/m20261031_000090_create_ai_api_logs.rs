use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// One row per call to an AI provider, successful or not, so a problem can be found afterwards.
const CREATE_AI_API_LOGS: &str = "
CREATE TABLE IF NOT EXISTS `ai_api_logs` (
  `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
  `kind` varchar(20) NOT NULL DEFAULT 'text',
  `provider` varchar(20) NOT NULL,
  `model` varchar(100) NOT NULL,
  `purpose` varchar(100) DEFAULT NULL,
  `status` varchar(20) NOT NULL,
  `http_status` smallint(5) unsigned DEFAULT NULL,
  `duration_ms` int(10) unsigned NOT NULL DEFAULT 0,
  `input_tokens` int(10) unsigned NOT NULL DEFAULT 0,
  `output_tokens` int(10) unsigned NOT NULL DEFAULT 0,
  `cost` double DEFAULT NULL,
  `image_count` smallint(5) unsigned NOT NULL DEFAULT 0,
  `sensitive` tinyint(1) NOT NULL DEFAULT 0,
  `prompt` longtext DEFAULT NULL,
  `response` longtext DEFAULT NULL,
  `error` text DEFAULT NULL,
  `created_at` timestamp NULL DEFAULT NULL,
  `updated_at` timestamp NULL DEFAULT NULL,
  PRIMARY KEY (`id`),
  KEY `ai_api_logs_status_created_at_index` (`status`,`created_at`),
  KEY `ai_api_logs_provider_index` (`provider`),
  KEY `ai_api_logs_created_at_index` (`created_at`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

/// The same table for PostgreSQL (no unsigned integers; `UnsignedInt` is `integer`).
const CREATE_AI_API_LOGS_POSTGRES: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS ai_api_logs (
  id bigserial PRIMARY KEY,
  kind varchar(20) NOT NULL DEFAULT 'text',
  provider varchar(20) NOT NULL,
  model varchar(100) NOT NULL,
  purpose varchar(100),
  status varchar(20) NOT NULL,
  http_status integer,
  duration_ms integer NOT NULL DEFAULT 0,
  input_tokens integer NOT NULL DEFAULT 0,
  output_tokens integer NOT NULL DEFAULT 0,
  cost double precision,
  image_count integer NOT NULL DEFAULT 0,
  sensitive boolean NOT NULL DEFAULT false,
  prompt text,
  response text,
  error text,
  created_at timestamptz,
  updated_at timestamptz
)",
    "CREATE INDEX IF NOT EXISTS ai_api_logs_status_created_at_index ON ai_api_logs (status, created_at)",
    "CREATE INDEX IF NOT EXISTS ai_api_logs_provider_index ON ai_api_logs (provider)",
    "CREATE INDEX IF NOT EXISTS ai_api_logs_created_at_index ON ai_api_logs (created_at)",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        loco_database::run_ddl(m, CREATE_AI_API_LOGS, CREATE_AI_API_LOGS_POSTGRES).await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        loco_database::drop_table_if_exists(m, "ai_api_logs").await
    }
}
