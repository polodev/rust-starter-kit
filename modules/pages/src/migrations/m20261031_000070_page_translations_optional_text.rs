use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// The text columns become optional so a language other than the default can leave any of them
/// empty; visitors then see the default language's text for that part. (Whether English has a title
/// and text is checked when a page is saved, not by the database.)
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.get_connection()
            .execute_unprepared(
                "ALTER TABLE `page_translations`
                   MODIFY `title` VARCHAR(255) NULL,
                   MODIFY `meta_description` VARCHAR(320) NULL,
                   MODIFY `body_html` TEXT NULL",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        let db = m.get_connection();
        db.execute_unprepared(
            "UPDATE `page_translations` SET
               `title` = COALESCE(`title`, ''),
               `meta_description` = COALESCE(`meta_description`, ''),
               `body_html` = COALESCE(`body_html`, '')",
        )
        .await?;
        db.execute_unprepared(
            "ALTER TABLE `page_translations`
               MODIFY `title` VARCHAR(255) NOT NULL,
               MODIFY `meta_description` VARCHAR(320) NOT NULL,
               MODIFY `body_html` TEXT NOT NULL",
        )
        .await?;
        Ok(())
    }
}
