use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Spatie laravel-medialibrary's `media` table, as Laravel creates it (with the later change of
/// `model_id` to text). `IF NOT EXISTS` makes this a no-op when a Laravel app already made it.
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

/// Carries over rows of the earlier `media_files` table. Their files sit at `media/{uuid}/{file}`,
/// so each row remembers that in `legacy_path`; the `is_public` flag becomes the `public` property.
const COPY_LEGACY: &str = "
INSERT INTO `media`
  (`model_type`, `model_id`, `uuid`, `collection_name`, `name`, `file_name`, `mime_type`, `disk`,
   `size`, `manipulations`, `custom_properties`, `responsive_images`, `order_column`, `created_at`, `updated_at`)
SELECT `model_type`, CAST(`model_id` AS CHAR), `uuid`, `collection_name`, `name`, `file_name`, `mime_type`, `disk`,
   `size`, '[]',
   JSON_SET(IF(JSON_VALID(`custom_properties`), `custom_properties`, '{}'),
            '$.legacy_path', CONCAT('media/', `uuid`, '/', `file_name`),
            '$.public', IF(`is_public`, 1, 0)),
   '[]', `order_column`, `created_at`, `updated_at`
FROM `media_files`";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        db.execute_unprepared(CREATE_MEDIA).await?;
        if m.has_table("media_files").await? {
            db.execute_unprepared(COPY_LEGACY).await?;
            db.execute_unprepared("DROP TABLE `media_files`").await?;
        }
        Ok(())
    }

    /// The `media` table may be shared with a Laravel app, so it is never dropped here.
    async fn down(&self, _m: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
