use loco_rs::{app::AppContext, testing::prelude::*, TestServer};
use module_pages::{page_translations, pages};
use rust_starter_pack_helpers::*;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serial_test::serial;

mod rust_starter_pack_helpers {
    use super::*;
    use rust_starter::{
        app::App,
        models::users::{self, RegisterParams},
    };
    use sea_orm::IntoActiveModel;

    pub const ORIGIN: &str = "http://localhost";

    pub async fn login(server: &TestServer, email: &str, password: &str) -> String {
        let response = server
            .post("/login")
            .add_header("host", "localhost")
            .add_header("origin", ORIGIN)
            .form(&[("email", email), ("password", password)])
            .await;
        assert_eq!(response.status_code(), 303);
        response
            .header("set-cookie")
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string()
    }

    pub async fn user(ctx: &AppContext, email: &str, role: Option<&str>) {
        module_role_permission::sync(&ctx.db).await.unwrap();
        let created = users::Model::create_with_password(
            &ctx.db,
            &RegisterParams {
                email: email.into(),
                password: "password123".into(),
                name: "Test Person".into(),
            },
        )
        .await
        .unwrap();
        let created = created.into_active_model().verified(&ctx.db).await.unwrap();
        if let Some(role) = role {
            module_role_permission::assign_role(&ctx.db, created.id, role)
                .await
                .unwrap();
        }
    }

    pub fn post(server: &TestServer, path: &str, cookie: &str) -> axum_test::TestRequest {
        server
            .post(path)
            .add_header("host", "localhost")
            .add_header("origin", ORIGIN)
            .add_header("cookie", cookie.to_string())
    }

    pub fn _app() -> std::marker::PhantomData<App> {
        std::marker::PhantomData
    }
}

async fn page_id(ctx: &AppContext, slug: &str) -> i64 {
    pages::Entity::find()
        .filter(pages::Column::Slug.eq(slug))
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap()
        .id
}

async fn translation(ctx: &AppContext, id: i64, locale: &str) -> Option<page_translations::Model> {
    page_translations::Entity::find()
        .filter(page_translations::Column::PageId.eq(id))
        .filter(page_translations::Column::Locale.eq(locale))
        .one(&ctx.db)
        .await
        .unwrap()
}

macro_rules! app {
    ($body:expr) => {
        request::<rust_starter::app::App, _, _>($body).await
    };
}

#[tokio::test]
#[serial]
async fn the_admin_show_page_is_a_read_only_view_of_both_languages() {
    app!(|server, ctx| async move {
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        user(&ctx, "market@example.com", Some("marketing")).await; // pages.manage
        user(&ctx, "biz@example.com", Some("business_development")).await; // pages.view
        user(&ctx, "plain@example.com", None).await;
        let id = page_id(&ctx, "about").await;
        let url = format!("/admin/pages/{id}");

        let marketer = login(&server, "market@example.com", "password123").await;
        let html = server.get(&url).add_header("cookie", marketer).await.text();
        assert!(html.contains(r#"data-testid="page-title""#) && html.contains("About us"));
        assert!(
            html.contains(r#"data-testid="content-en""#) && html.contains("What we do"),
            "the English text is shown"
        );
        assert!(
            html.contains(r#"data-testid="content-bn""#) && html.contains("আমাদের সম্পর্কে"),
            "the Bangla text is shown"
        );
        assert!(
            html.contains("<h2>What we do</h2>"),
            "the body is rendered as visitors see it"
        );
        assert!(
            html.contains(r#"data-testid="edit-button""#)
                && html.contains(&format!(r#"href="/admin/pages/{id}/edit""#)),
            "an Edit button for people who may edit"
        );
        assert!(html.contains("View on website"));

        // Look-only staff see the page but no Edit button.
        let viewer = login(&server, "biz@example.com", "password123").await;
        let html = server.get(&url).add_header("cookie", viewer).await;
        html.assert_status_ok();
        assert!(!html.text().contains(r#"data-testid="edit-button""#));

        let plain = login(&server, "plain@example.com", "password123").await;
        assert_eq!(
            server
                .get(&url)
                .add_header("cookie", plain)
                .await
                .status_code(),
            403
        );
        assert_eq!(
            server.get(&url).await.status_code(),
            303,
            "guests are sent to log in"
        );
        let dev = login(&server, "market@example.com", "password123").await;
        assert_eq!(
            server
                .get("/admin/pages/99999")
                .add_header("cookie", dev)
                .await
                .status_code(),
            404
        );
    });
}

#[tokio::test]
#[serial]
async fn the_edit_page_has_a_text_editor_for_each_language() {
    app!(|server, ctx| async move {
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        user(&ctx, "market@example.com", Some("marketing")).await;
        user(&ctx, "biz@example.com", Some("business_development")).await;
        let id = page_id(&ctx, "privacy-policy").await;
        let url = format!("/admin/pages/{id}/edit");

        let marketer = login(&server, "market@example.com", "password123").await;
        let html = server.get(&url).add_header("cookie", marketer).await.text();
        assert!(
            html.contains(r#"data-testid="content-editor""#)
                && html.contains(&format!(r#"action="/admin/pages/{id}/content""#))
        );
        for field in [
            "title-en", "meta-en", "body-en", "title-bn", "meta-bn", "body-bn",
        ] {
            assert!(
                html.contains(&format!(r#"data-testid="{field}""#)),
                "missing the {field} field"
            );
        }
        assert!(
            html.contains("Privacy Policy") && html.contains("গোপনীয়তা নীতি"),
            "the fields start with the saved text"
        );
        assert!(
            html.contains("1. Information we collect"),
            "the body textarea holds the current text"
        );
        assert!(html.contains(r#"data-testid="save-content""#));

        let viewer = login(&server, "biz@example.com", "password123").await;
        let html = server.get(&url).add_header("cookie", viewer).await.text();
        assert!(
            !html.contains(r#"data-testid="save-content""#)
                && html.contains("You can read this page but not edit it")
        );
    });
}

#[tokio::test]
#[serial]
async fn saving_the_text_updates_the_public_page_at_once_and_cleans_unsafe_html() {
    app!(|server, ctx| async move {
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        user(&ctx, "market@example.com", Some("marketing")).await;
        let id = page_id(&ctx, "about").await;
        let marketer = login(&server, "market@example.com", "password123").await;

        // Warm the cache for both languages.
        assert!(server.get("/pages/about").await.text().contains("About us"));
        assert!(server
            .get("/pages/about?lang=bn")
            .await
            .text()
            .contains("আমাদের সম্পর্কে"));

        let saved = post(&server, &format!("/admin/pages/{id}/content"), &marketer)
            .form(&[
                ("title_en", "  About our team "),
                ("meta_en", "Who we are."),
                ("body_en", r#"<h2>Hello</h2><p>We are <strong>great</strong>.<script>alert(1)</script></p><p onclick="steal()">Click <a href="javascript:evil()">here</a> or <a href="https://example.com">there</a></p>"#),
                ("title_bn", "আমাদের দল"),
                ("meta_bn", "আমরা কারা।"),
                ("body_bn", "<p>নমস্কার, আমরা <em>প্রস্তুত</em>।</p>"),
            ])
            .await;
        assert_eq!(saved.status_code(), 303);
        assert_eq!(
            saved.header("location"),
            format!("/admin/pages/{id}/edit?saved=content")
        );

        let english = server.get("/pages/about").await.text();
        assert!(
            english.contains("About our team") && english.contains("<strong>great</strong>"),
            "visitors see the new text straight away"
        );
        assert!(
            !english.contains("alert(1)")
                && !english.contains("steal()")
                && !english.contains("javascript:"),
            "unsafe HTML is removed"
        );
        assert!(
            english.contains(r#"href="https://example.com""#),
            "safe links stay"
        );
        assert!(
            english.contains(r#"content="Who we are.""#),
            "the search description is updated"
        );
        let bangla = server.get("/pages/about?lang=bn").await.text();
        assert!(bangla.contains("আমাদের দল") && bangla.contains("<em>প্রস্তুত</em>"));

        let stored = translation(&ctx, id, "en").await.unwrap();
        assert_eq!(
            stored.title.as_deref(),
            Some("About our team"),
            "the title is trimmed"
        );
        assert!(
            !stored
                .body_html
                .clone()
                .unwrap_or_default()
                .contains("<script")
                && !stored
                    .body_html
                    .clone()
                    .unwrap_or_default()
                    .contains("onclick"),
            "the database holds the cleaned HTML"
        );

        let message = server
            .get(&format!("/admin/pages/{id}/edit?saved=content"))
            .add_header("cookie", marketer)
            .await
            .text();
        assert!(message.contains("Page content saved."));
    });
}

#[tokio::test]
#[serial]
async fn invalid_text_saves_nothing_and_shows_what_was_typed() {
    app!(|server, ctx| async move {
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        user(&ctx, "market@example.com", Some("marketing")).await;
        let id = page_id(&ctx, "about").await;
        let marketer = login(&server, "market@example.com", "password123").await;
        let path = format!("/admin/pages/{id}/content");

        // English has no title: nothing is saved, not even the good Bangla text.
        let response = post(&server, &path, &marketer)
            .form(&[
                ("title_en", ""),
                ("meta_en", ""),
                ("body_en", "<p>Changed body</p>"),
                ("title_bn", "নতুন শিরোনাম"),
                ("meta_bn", ""),
                ("body_bn", "<p>বাংলা</p>"),
            ])
            .await;
        assert_eq!(response.status_code(), 422);
        let html = response.text();
        assert!(html.contains("English: the title is required."), "{html}");
        assert!(
            html.contains("&lt;p&gt;Changed body&lt;/p&gt;") && html.contains("নতুন শিরোনাম"),
            "what was typed is shown again, HTML-escaped inside the textarea"
        );
        assert_eq!(
            translation(&ctx, id, "en").await.unwrap().title,
            Some("About us".to_string()),
            "nothing was saved"
        );

        for (form, message) in [
            (
                vec![
                    ("title_en", "x".repeat(256)),
                    ("body_en", "<p>a</p>".into()),
                ],
                "English: the title is longer than 255 characters.",
            ),
            (
                vec![
                    ("title_en", "Ok".into()),
                    ("meta_en", "m".repeat(321)),
                    ("body_en", "<p>a</p>".into()),
                ],
                "English: the description is longer than 320 characters.",
            ),
            (
                vec![("title_en", "Ok".into()), ("body_en", "   ".into())],
                "English: the page text is required.",
            ),
            (
                vec![
                    ("title_en", "Ok".into()),
                    ("body_en", format!("<p>{}</p>", "a".repeat(61_000))),
                ],
                "English: the page text is too long.",
            ),
        ] {
            let mut fields = vec![
                ("title_bn".to_string(), "বাংলা শিরোনাম".to_string()),
                ("body_bn".to_string(), "<p>ঠিক আছে</p>".to_string()),
            ];
            fields.extend(form.into_iter().map(|(k, v)| (k.to_string(), v)));
            let response = post(&server, &path, &marketer).form(&fields).await;
            assert_eq!(response.status_code(), 422, "{message}");
            assert!(response.text().contains(message), "expected `{message}`");
        }
        assert_eq!(
            translation(&ctx, id, "bn").await.unwrap().title,
            Some("আমাদের সম্পর্কে".to_string()),
            "still the original Bangla title"
        );
    });
}

#[tokio::test]
#[serial]
async fn a_missing_language_can_be_written_later() {
    app!(|server, ctx| async move {
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        user(&ctx, "market@example.com", Some("marketing")).await;
        let marketer = login(&server, "market@example.com", "password123").await;

        let page = pages::ActiveModel {
            slug: Set("news".into()),
            is_published: Set(true),
            sort_order: Set(9),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        page_translations::ActiveModel {
            page_id: Set(page.id),
            locale: Set("en".into()),
            title: Set(Some("News".into())),
            meta_description: Set(None),
            body_html: Set(Some("<p>English only</p>".into())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        let show = server
            .get(&format!("/admin/pages/{}", page.id))
            .add_header("cookie", marketer.clone())
            .await
            .text();
        assert!(
            show.contains("(uses English)")
                && show.contains("No বাংলা version: visitors see the English text."),
            "the show page says the missing language uses English"
        );
        // Visitors who choose Bangla still get the English text.
        assert!(server
            .get("/pages/news?lang=bn")
            .await
            .text()
            .contains("English only"));

        let response = post(
            &server,
            &format!("/admin/pages/{}/content", page.id),
            &marketer,
        )
        .form(&[
            ("title_en", "News"),
            ("body_en", "<p>English only</p>"),
            ("title_bn", "খবর"),
            ("body_bn", "<p>শুধু বাংলা নয়</p>"),
        ])
        .await;
        assert_eq!(response.status_code(), 303);
        assert_eq!(
            translation(&ctx, page.id, "bn").await.unwrap().title,
            Some("খবর".to_string()),
            "the new language was created"
        );
        assert!(server
            .get("/pages/news?lang=bn")
            .await
            .text()
            .contains("শুধু বাংলা নয়"));
    });
}

#[tokio::test]
#[serial]
async fn only_people_who_may_manage_pages_can_save_text() {
    app!(|server, ctx| async move {
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        user(&ctx, "market@example.com", Some("marketing")).await;
        user(&ctx, "biz@example.com", Some("business_development")).await;
        user(&ctx, "plain@example.com", None).await;
        let id = page_id(&ctx, "about").await;
        let path = format!("/admin/pages/{id}/content");
        let form = [("title_en", "Hacked"), ("body_en", "<p>x</p>")];

        for email in ["biz@example.com", "plain@example.com"] {
            let cookie = login(&server, email, "password123").await;
            assert_eq!(
                post(&server, &path, &cookie)
                    .form(&form)
                    .await
                    .status_code(),
                403,
                "{email}"
            );
        }
        assert_eq!(
            server
                .post(&path)
                .add_header("host", "localhost")
                .add_header("origin", ORIGIN)
                .form(&form)
                .await
                .status_code(),
            303,
            "guests are sent to log in"
        );

        let marketer = login(&server, "market@example.com", "password123").await;
        let foreign = server
            .post(&path)
            .add_header("host", "localhost")
            .add_header("origin", "https://evil.example")
            .add_header("cookie", marketer)
            .form(&form)
            .await;
        assert_eq!(
            foreign.status_code(),
            403,
            "other sites cannot post as a staff member"
        );
        assert_eq!(
            translation(&ctx, id, "en").await.unwrap().title,
            Some("About us".to_string()),
            "nothing changed"
        );
    });
}

#[tokio::test]
#[serial]
async fn the_page_list_links_to_the_show_page_and_the_editor() {
    app!(|server, ctx| async move {
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        user(&ctx, "market@example.com", Some("marketing")).await;
        user(&ctx, "biz@example.com", Some("business_development")).await;
        let id = page_id(&ctx, "about").await;

        let marketer = login(&server, "market@example.com", "password123").await;
        let html = server
            .get("/admin/pages")
            .add_header("cookie", marketer)
            .await
            .text();
        assert!(
            html.contains(&format!(r#"href="/admin/pages/{id}""#))
                && html.contains(r#"data-testid="page-link""#),
            "the title opens the show page"
        );
        assert!(
            html.contains(&format!(r#"href="/admin/pages/{id}/edit""#))
                && html.contains(r#"data-testid="edit-link""#)
        );

        let viewer = login(&server, "biz@example.com", "password123").await;
        let html = server
            .get("/admin/pages")
            .add_header("cookie", viewer)
            .await
            .text();
        assert!(
            html.contains(&format!(r#"href="/admin/pages/{id}""#))
                && !html.contains(r#"data-testid="edit-link""#),
            "look-only staff get no Edit link"
        );
    });
}

#[tokio::test]
#[serial]
async fn bangla_is_optional_and_each_empty_part_uses_the_english_text() {
    app!(|server, ctx| async move {
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        user(&ctx, "market@example.com", Some("marketing")).await;
        let marketer = login(&server, "market@example.com", "password123").await;
        let id = page_id(&ctx, "about").await;
        let path = format!("/admin/pages/{id}/content");
        let save = |fields: Vec<(&'static str, &'static str)>| {
            let (server, marketer, path) = (&server, marketer.clone(), path.clone());
            async move {
                let mut all = vec![("title_en", "About us"), ("body_en", "<p>English text</p>")];
                all.extend(fields);
                post(server, &path, &marketer).form(&all).await
            }
        };

        // English stays required, whatever Bangla holds.
        let refused = post(&server, &path, &marketer)
            .form(&[
                ("title_en", ""),
                ("body_en", "<p>x</p>"),
                ("title_bn", "শিরোনাম"),
                ("body_bn", "<p>বাংলা</p>"),
            ])
            .await;
        assert_eq!(refused.status_code(), 422);
        assert!(refused.text().contains("English: the title is required."));

        // Nothing typed for Bangla: its row goes away and visitors see English.
        assert_eq!(save(vec![]).await.status_code(), 303);
        assert!(
            translation(&ctx, id, "bn").await.is_none(),
            "a cleared language has no row"
        );
        let page = server.get("/pages/about?lang=bn").await.text();
        assert!(page.contains("About us") && page.contains("English text"));

        // Only a Bangla title: the title is Bangla, the text is English.
        assert_eq!(
            save(vec![("title_bn", "আমাদের পরিচয়")]).await.status_code(),
            303
        );
        let stored = translation(&ctx, id, "bn").await.unwrap();
        assert_eq!(
            (
                stored.title.as_deref(),
                stored.meta_description.as_deref(),
                stored.body_html.as_deref()
            ),
            (Some("আমাদের পরিচয়"), None, None),
            "empty parts are stored as NULL"
        );
        let page = server.get("/pages/about?lang=bn").await.text();
        assert!(page.contains("আমাদের পরিচয়") && page.contains("English text"));
        let show = server
            .get(&format!("/admin/pages/{id}"))
            .add_header("cookie", marketer.clone())
            .await
            .text();
        assert!(show.contains("The English text is used."));

        // Only a Bangla text: the text is Bangla, the title is English.
        assert_eq!(
            save(vec![("body_bn", "<p>শুধু বাংলা লেখা</p>")])
                .await
                .status_code(),
            303
        );
        let page = server.get("/pages/about?lang=bn").await.text();
        assert!(page.contains("শুধু বাংলা লেখা") && page.contains("About us"));
        assert!(!page.contains("আমাদের পরিচয়"));

        // Bangla limits still apply to what is typed.
        let long = "ক".repeat(256);
        let too_long = post(&server, &path, &marketer)
            .form(&[
                ("title_en", "About us"),
                ("body_en", "<p>English text</p>"),
                ("title_bn", long.as_str()),
            ])
            .await;
        assert_eq!(too_long.status_code(), 422);
        assert!(too_long
            .text()
            .contains("বাংলা: the title is longer than 255 characters."));
    });
}
