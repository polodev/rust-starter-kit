use fluent_templates::{ArcLoader, FluentLoader};
use loco_rs::controller::views::{engines, ViewRenderer};

/// Renders every module template through the same view engine the app builds at boot,
/// including the Fluent `t()` function. Tera 2 validates function references when templates
/// are loaded, so a registration-ordering mistake fails here rather than at runtime.
fn module_view_engine() -> engines::TeraView {
    let loader = std::sync::Arc::new(
        ArcLoader::builder("assets/i18n", unic_langid::langid!("en-US"))
            .shared_resources(Some(&["assets/shared.ftl".into()]))
            .customize(|bundle| bundle.set_use_isolating(false))
            .build()
            .expect("locales should load"),
    );
    let templates = loco_modular::collect_templates(&modules_registry::all());

    engines::TeraView::build_with_post_process(move |tera| {
        tera.register_function("t", FluentLoader::new(loader.clone()));
        tera.add_raw_templates(templates.iter().copied())
            .map_err(|e| loco_rs::Error::string(&e.to_string()))?;
        Ok(())
    })
    .expect("view engine should build")
}

#[test]
fn renders_home_view_in_english_and_bangla() {
    let view = module_view_engine();

    let english = view
        .render(
            "site/home.html",
            serde_json::json!({ "lang": "en", "tag": "en-US", "path": "/" }),
        )
        .expect("english home should render");
    assert!(
        english.contains("Welcome to your new website"),
        "got: {english}"
    );

    let bangla = view
        .render(
            "site/home.html",
            serde_json::json!({ "lang": "bn", "tag": "bn-BD", "path": "/" }),
        )
        .expect("bangla home should render");
    assert!(bangla.contains("আপনার নতুন ওয়েবসাইটে স্বাগতম"), "got: {bangla}");
}

#[test]
fn every_translation_key_exists_in_both_languages() {
    let keys = |path: &str| -> std::collections::BTreeSet<String> {
        std::fs::read_to_string(path)
            .expect("ftl file should exist")
            .lines()
            .filter_map(|line| {
                line.split_once(" = ")
                    .map(|(key, _)| key.trim().to_string())
            })
            .collect()
    };
    assert_eq!(
        keys("assets/i18n/en-US/main.ftl"),
        keys("assets/i18n/bn-BD/main.ftl")
    );
}
