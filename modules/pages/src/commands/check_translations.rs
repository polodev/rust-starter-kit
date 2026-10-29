use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Error, Result,
};
use module_site_layout::SUPPORTED_LOCALES;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use crate::entities::{page_translations, pages};

/// Reports published pages that lack a translation in a supported language.
pub struct CheckTranslations;

#[async_trait]
impl Task for CheckTranslations {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "pages:check-translations".to_string(),
            detail: "List published pages that miss a translation. Add `strict:true` to fail when any are missing.\nUsage: cargo run -- task pages:check-translations strict:true".to_string(),
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
                .map(|translation| translation.locale)
                .collect();
            let missing: Vec<&str> = SUPPORTED_LOCALES
                .into_iter()
                .filter(|locale| !present.iter().any(|p| p == locale))
                .collect();

            if missing.is_empty() {
                println!("ok       {:<20} {}", page.slug, present.join(", "));
            } else {
                missing_total += missing.len();
                println!("MISSING  {:<20} needs {}", page.slug, missing.join(", "));
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
