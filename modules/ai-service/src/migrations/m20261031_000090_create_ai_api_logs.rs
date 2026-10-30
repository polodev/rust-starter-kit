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

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.get_connection()
            .execute_unprepared(CREATE_AI_API_LOGS)
            .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS `ai_api_logs`")
            .await?;
        Ok(())
    }
}
