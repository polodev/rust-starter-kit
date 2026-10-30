use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Error, Result,
};
use module_site_layout::{DEFAULT_LOCALE, SUPPORTED_LOCALES};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::entities::{page_translations, pages};

/// Reports published pages that lack a translation in a supported language.
pub struct CheckTranslations;

#[async_trait]
impl Task for CheckTranslations {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "pages:check-translations".to_string(),
            detail: "List published pages that miss their English text, and the pages that use English for another language. Add `strict:true` to fail when English is missing.\nUsage: cargo run -- task pages:check-translations strict:true".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let strict = vars.cli_arg("strict").is_ok_and(|value| value == "true");

        let published = pages::Entity::find()
            .filter(pages::Column::IsPublished.eq(true))
            .all(&ctx.db)
            .await?;
        let mut missing_total = 0;

        for page in published {
            let present: Vec<String> = page_translations::Entity::find()
                .filter(page_translations::Column::PageId.eq(page.id))
                .all(&ctx.db)
                .await?
                .into_iter()
                .filter(|translation| {
                    translation.title.is_some() && translation.body_html.is_some()
                })
                .map(|translation| translation.locale)
                .collect();

            // English is required. Other languages are optional and fall back to English.
            if !present.iter().any(|locale| locale == DEFAULT_LOCALE) {
                missing_total += 1;
                println!("MISSING  {:<20} needs {DEFAULT_LOCALE}", page.slug);
                continue;
            }
            let fallback: Vec<&str> = SUPPORTED_LOCALES
                .into_iter()
                .filter(|locale| !present.iter().any(|p| p == locale))
                .collect();
            if fallback.is_empty() {
                println!("ok       {:<20} {}", page.slug, present.join(", "));
            } else {
                println!(
                    "ok       {:<20} {} (uses English for {})",
                    page.slug,
                    present.join(", "),
                    fallback.join(", ")
                );
            }
        }

        if strict && missing_total > 0 {
            return Err(Error::string(&format!(
                "{missing_total} translation(s) missing"
            )));
        }
        Ok(())
    }
}
