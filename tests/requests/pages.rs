use loco_rs::{testing::prelude::*, TestServer};
use rstest::rstest;
use rust_starter::app::App;
use serial_test::serial;

async fn seeded<F, Fut>(test: F)
where
    F: FnOnce(TestServer) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    request::<App, _, _>(|server, ctx| async move {
        seed::<App>(&ctx).await.unwrap();
        test(server).await;
    })
    .await;
}

#[rstest]
#[case("about", "About us", "আমাদের সম্পর্কে")]
#[case("privacy-policy", "Privacy Policy", "গোপনীয়তা নীতি")]
#[case("terms-of-service", "Terms of Service", "পরিষেবার শর্তাবলি")]
#[tokio::test]
#[serial]
async fn seeded_pages_render_in_both_languages(
    #[case] slug: &str,
    #[case] english: &str,
    #[case] bangla: &str,
) {
    seeded(|server| async move {
        let path = format!("/pages/{slug}");

        let response = server.get(&path).await;
        response.assert_status_ok();
        assert!(response.text().contains(english));
        assert!(response.text().contains(r#"<html lang="en""#));

        let response = server.get(&format!("{path}?lang=bn")).await;
        response.assert_status_ok();
        assert!(response.text().contains(bangla));
        assert!(response.text().contains(r#"<html lang="bn""#));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn language_cookie_and_accept_language_select_bangla() {
    seeded(|server| async move {
        let response = server
            .get("/pages/about")
            .add_header("cookie", "lang=bn")
            .await;
        assert!(response.text().contains("আমাদের সম্পর্কে"));

        let response = server
            .get("/pages/about")
            .add_header("accept-language", "bn-BD,bn;q=0.9")
            .await;
        assert!(response.text().contains("আমাদের সম্পর্কে"));

        // The query string beats the cookie.
        let response = server
            .get("/pages/about?lang=en")
            .add_header("cookie", "lang=bn")
            .await;
        assert!(response.text().contains("About us"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn unknown_page_is_a_translated_404() {
    seeded(|server| async move {
        let response = server.get("/pages/missing").await;
        response.assert_status_not_found();
        assert!(response.text().contains("Page not found"));

        let response = server
            .get("/pages/missing")
            .add_header("cookie", "lang=bn")
            .await;
        response.assert_status_not_found();
        assert!(response.text().contains("পৃষ্ঠাটি পাওয়া যায়নি"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn language_switch_sets_a_year_long_cookie_and_returns_to_the_page() {
    seeded(|server| async move {
        let response = server.get("/language/bn?next=/pages/about").await;
        response.assert_status(axum::http::StatusCode::SEE_OTHER);
        assert_eq!(response.header("location"), "/pages/about");
        let cookie = response.header("set-cookie").to_str().unwrap().to_string();
        assert!(cookie.contains("lang=bn"));
        assert!(cookie.contains("Max-Age=31536000"));
    })
    .await;
}

#[rstest]
#[case("//evil.example")]
#[case("https://evil.example")]
#[case("/\\evil")]
#[tokio::test]
#[serial]
async fn language_switch_never_redirects_off_site(#[case] next: &str) {
    seeded(|server| async move {
        let response = server
            .get("/language/en")
            .add_query_param("next", next)
            .await;
        assert_eq!(response.header("location"), "/");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn unsupported_language_is_rejected() {
    seeded(|server| async move {
        server.get("/language/fr").await.assert_status_not_found();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn seeding_twice_does_not_duplicate_pages() {
    request::<App, _, _>(|server, ctx| async move {
        let modules = modules_registry::all();
        loco_modular::seed_all(&modules, &ctx).await.unwrap();
        loco_modular::seed_all(&modules, &ctx).await.unwrap();
        server.get("/pages/about").await.assert_status_ok();

        use sea_orm::{EntityTrait, PaginatorTrait};
        assert_eq!(
            module_pages::pages::Entity::find()
                .count(&ctx.db)
                .await
                .unwrap(),
            3
        );
        assert_eq!(
            module_pages::page_translations::Entity::find()
                .count(&ctx.db)
                .await
                .unwrap(),
            6
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn header_has_the_language_dropdown_and_theme_toggle() {
    seeded(|server| async move {
        let html = server.get("/pages/about").await.text();

        // Dropdown lists both languages and returns to the current page.
        assert!(html.contains(r#"href="/language/bn?next=/pages/about""#));
        assert!(html.contains(r#"href="/language/en?next=/pages/about""#));
        assert!(html.contains("Interface language"));
        assert!(html.contains(r#"x-data="{ open: false }""#));

        // Theme toggle and the pre-paint script that applies the saved theme.
        assert!(html.contains(r#"data-testid="theme-toggle""#));
        assert!(html.contains("window.setAppearance"));
        assert!(html.contains("Toggle theme"));

        let html = server.get("/pages/about?lang=bn").await.text();
        assert!(html.contains("ইন্টারফেসের ভাষা"));
        assert!(html.contains("থিম পরিবর্তন"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn alpine_and_dark_mode_css_are_served() {
    seeded(|server| async move {
        server
            .get("/static/js/alpine.min.js")
            .await
            .assert_status_ok();
        let css = server.get("/static/css/app.css").await.text();
        assert!(
            css.contains(r"dark\:bg-gray-900"),
            "dark variants must be compiled into app.css"
        );
    })
    .await;
}
