use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_MAIL_LOG: &str = "
CREATE TABLE IF NOT EXISTS `mail_log` (
  `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
  `recipients` varchar(1000) NOT NULL,
  `from_address` varchar(255) NOT NULL,
  `subject` varchar(255) NOT NULL,
  `driver` varchar(20) NOT NULL,
  `status` varchar(16) NOT NULL,
  `attempts` int(11) NOT NULL DEFAULT 0,
  `error` text DEFAULT NULL,
  `provider_id` varchar(255) DEFAULT NULL,
  `attachments` varchar(1000) DEFAULT NULL,
  `sent_at` timestamp NULL DEFAULT NULL,
  `created_at` timestamp NULL DEFAULT NULL,
  `updated_at` timestamp NULL DEFAULT NULL,
  PRIMARY KEY (`id`),
  KEY `mail_log_status_index` (`status`),
  KEY `mail_log_created_at_index` (`created_at`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.get_connection()
            .execute_unprepared(CREATE_MAIL_LOG)
            .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS `mail_log`")
            .await?;
        Ok(())
    }
}
