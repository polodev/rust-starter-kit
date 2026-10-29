use std::time::Duration;

use loco_modular::remember;
use loco_rs::prelude::*;
use module_site_layout::{Site, DEFAULT_LOCALE};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

use crate::entities::{page_translations, pages};

/// How long a rendered page row stays in the short-lived cache.
const PAGE_CACHE_TTL: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PageView {
    title: String,
    meta_description: String,
    body_html: String,
    /// Locale of the translation actually returned (may fall back to the default).
    locale: String,
}

/// Finds the published page in the requested language, falling back to the default language.
async fn find_page(db: &DatabaseConnection, slug: &str, locale: &str) -> Result<Option<PageView>> {
    let Some(page) = pages::Entity::find()
        .filter(pages::Column::Slug.eq(slug))
        .filter(pages::Column::IsPublished.eq(true))
        .one(db)
        .await?
    else {
        return Ok(None);
    };

    for candidate in [locale, DEFAULT_LOCALE] {
        if let Some(translation) = page_translations::Entity::find()
            .filter(page_translations::Column::PageId.eq(page.id))
            .filter(page_translations::Column::Locale.eq(candidate))
            .one(db)
            .await?
        {
            return Ok(Some(PageView {
                title: translation.title,
                meta_description: translation.meta_description,
                body_html: translation.body_html,
                locale: translation.locale,
            }));
        }
    }
    Ok(None)
}

#[debug_handler]
async fn show(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    Path(slug): Path<String>,
    site: Site,
) -> Result<Response> {
    let code = site.lang();

    let cache_key = format!("pages:{slug}:{code}");
    let page = remember(&ctx, &cache_key, PAGE_CACHE_TTL, async {
        find_page(&ctx.db, &slug, code).await
    })
    .await?;

    match page {
        Some(page) => format::render().view(
            &v,
            "pages/show.html",
            site.data(serde_json::json!({ "page": page })),
        ),
        None => {
            let body = v.render("site/not_found.html", site.data(serde_json::json!({})))?;
            Ok((
                axum::http::StatusCode::NOT_FOUND,
                axum::response::Html(body),
            )
                .into_response())
        }
    }
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new().prefix("pages").add("/{slug}", get(show))]
}
