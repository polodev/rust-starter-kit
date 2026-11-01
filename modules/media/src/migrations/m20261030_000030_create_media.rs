use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Spatie laravel-medialibrary's `media` table, as Laravel creates it (with `model_id` as text so
/// models with uuid keys work). `IF NOT EXISTS` makes this a no-op when a Laravel app that shares
/// the database already made it.
const CREATE_MEDIA: &str = "
CREATE TABLE IF NOT EXISTS `media` (
  `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
  `model_type` varchar(191) NOT NULL,
  `model_id` varchar(50) NOT NULL,
  `uuid` char(36) DEFAULT NULL,
  `collection_name` varchar(191) NOT NULL,
  `name` varchar(191) NOT NULL,
  `file_name` varchar(191) NOT NULL,
  `mime_type` varchar(191) DEFAULT NULL,
  `disk` varchar(191) NOT NULL,
  `conversions_disk` varchar(191) DEFAULT NULL,
  `size` int(10) unsigned NOT NULL,
  `manipulations` longtext NOT NULL,
  `custom_properties` longtext NOT NULL,
  `generated_conversions` longtext DEFAULT NULL,
  `responsive_images` longtext NOT NULL,
  `order_column` int(10) unsigned DEFAULT NULL,
  `created_at` timestamp NULL DEFAULT NULL,
  `updated_at` timestamp NULL DEFAULT NULL,
  PRIMARY KEY (`id`),
  UNIQUE KEY `media_uuid_unique` (`uuid`),
  KEY `media_model_type_model_id_index` (`model_type`,`model_id`),
  KEY `media_order_column_index` (`order_column`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";

/// The same table for PostgreSQL (Spatie's PostgreSQL layout: `integer` for the unsigned columns).
const CREATE_MEDIA_POSTGRES: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS media (
  id bigserial PRIMARY KEY,
  model_type varchar(191) NOT NULL,
  model_id varchar(50) NOT NULL,
  uuid varchar(36),
  collection_name varchar(191) NOT NULL,
  name varchar(191) NOT NULL,
  file_name varchar(191) NOT NULL,
  mime_type varchar(191),
  disk varchar(191) NOT NULL,
  conversions_disk varchar(191),
  size integer NOT NULL,
  manipulations text NOT NULL,
  custom_properties text NOT NULL,
  generated_conversions text,
  responsive_images text NOT NULL,
  order_column integer,
  created_at timestamptz,
  updated_at timestamptz
)",
    "CREATE UNIQUE INDEX IF NOT EXISTS media_uuid_unique ON media (uuid)",
    "CREATE INDEX IF NOT EXISTS media_model_type_model_id_index ON media (model_type, model_id)",
    "CREATE INDEX IF NOT EXISTS media_order_column_index ON media (order_column)",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        loco_database::run_ddl(m, CREATE_MEDIA, CREATE_MEDIA_POSTGRES).await
    }

    /// The `media` table may be shared with a Laravel app, so it is never dropped here.
    async fn down(&self, _m: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
