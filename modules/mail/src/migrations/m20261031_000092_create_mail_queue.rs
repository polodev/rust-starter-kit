use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Emails waiting to be sent by `mail:work`, in the spirit of Laravel's `jobs` table. `mail_log` keeps the history;
// a row here is removed once the mail is sent or has used all its attempts.
const CREATE_MAIL_QUEUE: &str = "
CREATE TABLE IF NOT EXISTS `mail_queue` (
  `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
  `mail_log_id` bigint(20) unsigned NOT NULL,
  `payload` longtext NOT NULL,
  `attempts` int(11) NOT NULL DEFAULT 0,
  `available_at` timestamp NOT NULL,
  `reserved_at` timestamp NULL DEFAULT NULL,
  `created_at` timestamp NULL DEFAULT NULL,
  `updated_at` timestamp NULL DEFAULT NULL,
  PRIMARY KEY (`id`),
  KEY `mail_queue_available_at_index` (`available_at`),
  KEY `mail_queue_mail_log_id_index` (`mail_log_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

/// The same table for PostgreSQL.
const CREATE_MAIL_QUEUE_POSTGRES: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS mail_queue (
  id bigserial PRIMARY KEY,
  mail_log_id bigint NOT NULL,
  payload text NOT NULL,
  attempts integer NOT NULL DEFAULT 0,
  available_at timestamptz NOT NULL,
  reserved_at timestamptz,
  created_at timestamptz,
  updated_at timestamptz
)",
    "CREATE INDEX IF NOT EXISTS mail_queue_available_at_index ON mail_queue (available_at)",
    "CREATE INDEX IF NOT EXISTS mail_queue_mail_log_id_index ON mail_queue (mail_log_id)",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        loco_database::run_ddl(m, CREATE_MAIL_QUEUE, CREATE_MAIL_QUEUE_POSTGRES).await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        loco_database::drop_table_if_exists(m, "mail_queue").await
    }
}
