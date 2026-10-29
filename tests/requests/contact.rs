use loco_rs::{app::AppContext, testing::prelude::*, TestServer};
use module_contact::contacts;
use rust_starter_pack_helpers::*;
use sea_orm::{EntityTrait, PaginatorTrait};
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

fn public_post(server: &TestServer, ip: &str) -> axum_test::TestRequest {
    server
        .post("/contact")
        .add_header("host", "localhost")
        .add_header("origin", ORIGIN)
        .add_header("x-forwarded-for", ip.to_string())
}

const VALID: [(&str, &str); 5] = [
    ("name", "Rina Akter"),
    ("email", "Rina@Example.com"),
    ("subject", "Question"),
    ("message", "Do you ship to Chattogram?"),
    ("website", ""),
];

async fn stored(ctx: &AppContext) -> u64 {
    contacts::Entity::find().count(&ctx.db).await.unwrap()
}

#[tokio::test]
#[serial]
async fn the_public_form_is_a_component_that_answers_without_the_page() {
    request::<rust_starter::app::App, _, _>(|server, ctx| async move {
        let page = server.get("/contact").await.text();
        assert!(page.contains(r#"data-testid="contact-component""#) && page.contains("<html"));
        assert!(
            page.contains(r#"data-testid="contact-honeypot""#)
                && page.contains("/static/js/components.js")
        );

        // The component script asks for a fragment: only the component comes back.
        let fragment = public_post(&server, "10.0.0.1")
            .add_header("x-component", "1")
            .form(&VALID)
            .await;
        fragment.assert_status_ok();
        let html = fragment.text();
        assert!(html.contains(r#"data-testid="contact-success""#) && !html.contains("<html"));
        assert_eq!(stored(&ctx).await, 1);
        let saved = contacts::Entity::find()
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (saved.email.as_str(), saved.status.as_str()),
            ("rina@example.com", "new")
        );

        // Without the script a normal post gets the whole page back, so the form still works.
        let full = public_post(&server, "10.0.0.2").form(&VALID).await.text();
        assert!(full.contains("<html") && full.contains(r#"data-testid="contact-success""#));
        assert_eq!(stored(&ctx).await, 2);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn the_honeypot_looks_like_success_but_stores_nothing() {
    request::<rust_starter::app::App, _, _>(|server, ctx| async move {
        let response = public_post(&server, "10.0.0.3")
            .add_header("x-component", "1")
            .form(&[
                ("name", "Bot"),
                ("email", "bot@example.com"),
                ("subject", ""),
                ("message", "Buy cheap pills now!!"),
                ("website", "http://spam.example"),
            ])
            .await;
        response.assert_status_ok();
        assert!(response.text().contains(r#"data-testid="contact-success""#));
        assert_eq!(stored(&ctx).await, 0);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn invalid_messages_keep_the_values_and_show_errors() {
    request::<rust_starter::app::App, _, _>(|server, ctx| async move {
        let response = public_post(&server, "10.0.0.4")
            .add_header("x-component", "1")
            .form(&[
                ("name", "R"),
                ("email", "nope"),
                ("subject", ""),
                ("message", "short"),
                ("website", ""),
            ])
            .await;
        assert_eq!(response.status_code(), 422);
        let html = response.text();
        for key in ["error-name", "error-email", "error-message"] {
            assert!(html.contains(&format!(r#"data-testid="{key}""#)), "{key}");
        }
        assert!(html.contains(r#"value="nope""#), "typed values are kept");
        assert_eq!(stored(&ctx).await, 0);

        let foreign = server
            .post("/contact")
            .add_header("host", "localhost")
            .add_header("origin", "https://evil.example")
            .form(&VALID)
            .await;
        assert_eq!(foreign.status_code(), 403);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn one_address_cannot_flood_the_inbox() {
    request::<rust_starter::app::App, _, _>(|server, ctx| async move {
        for _ in 0..5 {
            public_post(&server, "10.0.0.9")
                .form(&VALID)
                .await
                .assert_status_ok();
        }
        let blocked = public_post(&server, "10.0.0.9").form(&VALID).await;
        assert_eq!(blocked.status_code(), 429);
        assert_eq!(stored(&ctx).await, 5);
        public_post(&server, "10.0.0.10")
            .form(&VALID)
            .await
            .assert_status_ok();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn admin_opens_a_message_and_marks_it_replied() {
    request::<rust_starter::app::App, _, _>(|server, ctx| async move {
        public_post(&server, "10.0.0.5").form(&VALID).await;
        let id = contacts::Entity::find()
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap()
            .id;
        user(&ctx, "boss@example.com", Some("developer")).await;
        let cookie = login(&server, "boss@example.com", "password123").await;

        let list = server
            .get("/admin/contacts")
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(
            list.contains(&format!(r#"data-testid="contact-row-{id}""#))
                && list.contains("Not replied")
        );
        assert!(list.contains(r#"data-testid="breadcrumb""#));

        let modal = server
            .get(&format!("/admin/contacts/{id}"))
            .add_header("cookie", cookie.clone())
            .add_header("x-component", "1")
            .await
            .text();
        assert!(
            modal.contains("Do you ship to Chattogram?")
                && modal.contains(r#"data-testid="mark-replied""#)
        );
        assert!(!modal.contains("<html"), "the modal gets only the fragment");

        let after = post(&server, &format!("/admin/contacts/{id}/status"), &cookie)
            .add_header("x-component", "1")
            .form(&[("status", "replied")])
            .await;
        after.assert_status_ok();
        let html = after.text();
        assert!(html.contains(r#"data-status="replied""#) && html.contains("Mark as not replied"));
        let row = contacts::Entity::find_by_id(id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.status, "replied");
        assert!(row.replied_at.is_some());

        let only_replied = server
            .get("/admin/contacts?status=replied")
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(only_replied.contains(&format!("contact-row-{id}")));
        let only_new = server
            .get("/admin/contacts?status=new")
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(!only_new.contains(&format!("contact-row-{id}")));

        post(&server, &format!("/admin/contacts/{id}/status"), &cookie)
            .form(&[("status", "new")])
            .await;
        assert_eq!(
            contacts::Entity::find_by_id(id)
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap()
                .status,
            "new"
        );
        let bad = post(&server, &format!("/admin/contacts/{id}/status"), &cookie)
            .form(&[("status", "weird")])
            .await;
        assert_eq!(bad.status_code(), 422);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn viewing_is_allowed_for_business_development_but_changing_is_not() {
    request::<rust_starter::app::App, _, _>(|server, ctx| async move {
        public_post(&server, "10.0.0.6").form(&VALID).await;
        let id = contacts::Entity::find()
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap()
            .id;
        user(&ctx, "bd@example.com", Some("business_development")).await;
        let cookie = login(&server, "bd@example.com", "password123").await;

        let modal = server
            .get(&format!("/admin/contacts/{id}"))
            .add_header("cookie", cookie.clone())
            .add_header("x-component", "1")
            .await;
        modal.assert_status_ok();
        assert!(!modal.text().contains(r#"data-testid="mark-replied""#));
        let change = post(&server, &format!("/admin/contacts/{id}/status"), &cookie)
            .form(&[("status", "replied")])
            .await;
        assert_eq!(change.status_code(), 403);
    })
    .await;
}
