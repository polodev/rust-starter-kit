use async_trait::async_trait;
use axum::{Extension, Router as AxumRouter};
use fluent_templates::{ArcLoader, FluentLoader};
use loco_rs::{
    app::{AppContext, Initializer},
    controller::views::{engines, ViewEngine},
    Error, Result,
};
use tracing::info;

const I18N_DIR: &str = "assets/i18n";
// NOTE: must live OUTSIDE `I18N_DIR` so the locale scan doesn't also pick it up;
// otherwise the shared resource is registered twice and bundle building fails
// ("Failed to add FTL resources to the bundle"). See loco-rs/loco#1749.
const I18N_SHARED: &str = "assets/shared.ftl";

/// Registers the Tera engine with Fluent translations (`t()`) and every module's templates.
#[allow(clippy::module_name_repetitions)]
pub struct ViewEngineInitializer;

#[async_trait]
impl Initializer for ViewEngineInitializer {
    fn name(&self) -> String {
        "view-engine".to_string()
    }

    async fn after_routes(&self, router: AxumRouter, _ctx: &AppContext) -> Result<AxumRouter> {
        let module_templates = loco_modular::collect_templates(&modules_registry::all());

        let arc = std::sync::Arc::new(
            ArcLoader::builder(&I18N_DIR, unic_langid::langid!("en-US"))
                .shared_resources(Some(&[I18N_SHARED.into()]))
                .customize(|bundle| bundle.set_use_isolating(false))
                .build()
                .map_err(|e| Error::string(&e.to_string()))?,
        );
        info!("locales loaded");

        let tera_engine = engines::TeraView::build_with_post_process(move |tera| {
            tera.register_function("t", FluentLoader::new(arc.clone()));
            // One batch, so `{% extends %}` between module templates resolves in any order.
            tera.add_raw_templates(module_templates.iter().copied())
                .map_err(|e| Error::string(&e.to_string()))?;
            Ok(())
        })?;

        Ok(router.layer(Extension(ViewEngine::from(tera_engine))))
    }
}
