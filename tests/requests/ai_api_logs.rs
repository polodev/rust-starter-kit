use loco_rs::{app::AppContext, testing::prelude::*, TestServer};
use module_ai_service::ai_api_logs;
use rust_starter_pack_helpers::*;
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, Set};
use serial_test::serial;

mod rust_starter_pack_helpers {
    use super::*;
    use rust_starter::models::users::{self, RegisterParams};
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
}

type App = rust_starter::app::App;

async fn seed(
    ctx: &AppContext,
    provider: &str,
    status: &str,
    purpose: &str,
    days_old: i64,
) -> ai_api_logs::Model {
    let at = chrono::Utc::now().fixed_offset() - chrono::Duration::days(days_old);
    ai_api_logs::ActiveModel {
        kind: Set("text".into()),
        provider: Set(provider.into()),
        model: Set("model-x".into()),
        purpose: Set(Some(purpose.into())),
        status: Set(status.into()),
        http_status: Set(Some(if status == "failed" { 429 } else { 200 })),
        duration_ms: Set(120),
        input_tokens: Set(10),
        output_tokens: Set(5),
        image_count: Set(0),
        sensitive: Set(false),
        prompt: Set(Some(format!("prompt for {purpose}"))),
        response: Set(Some("the reply".into())),
        error: Set((status == "failed").then(|| "rate limit reached".to_string())),
        created_at: Set(Some(at)),
        updated_at: Set(Some(at)),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
}

async fn count(ctx: &AppContext) -> u64 {
    ai_api_logs::Entity::find().count(&ctx.db).await.unwrap()
}

#[tokio::test]
#[serial]
async fn the_ai_api_logs_list_filters_pages_and_shows_one_call() {
    request::<App, _, _>(|server, ctx| async move {
        user(&ctx, "boss@example.com", Some("developer")).await;
        user(&ctx, "biz@example.com", Some("business_development")).await;
        let good = seed(&ctx, "claude", "success", "translate-page", 0).await;
        let bad = seed(&ctx, "openai", "failed", "passport-scan", 0).await;
        let cookie = login(&server, "boss@example.com", "password123").await;

        let list = server
            .get("/admin/ai-api-logs")
            .add_header("cookie", cookie.clone())
            .await;
        list.assert_status_ok();
        let html = list.text();
        assert!(html.contains("AI API logs") && html.contains(r#"data-testid="breadcrumb""#));
        assert!(html.contains(&format!(
            r#"href="/admin/ai-api-logs/{}" data-testid="id-link""#,
            good.id
        )));
        for length in ["25", "50", "100", "200", "500"] {
            assert!(
                html.contains(&format!(r#"<option value="{length}""#)),
                "page length {length}"
            );
        }
        assert!(
            html.contains(r#"data-testid="ai-api-logs-select-all""#)
                && html.contains(r#"data-testid="ai-api-logs-delete-selected""#)
        );

        let failed_only = server
            .get("/admin/ai-api-logs?status=failed")
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(failed_only.contains("passport-scan") && !failed_only.contains("translate-page"));
        let by_provider = server
            .get("/admin/ai-api-logs?provider=anthropic")
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(
            by_provider.contains("translate-page") && !by_provider.contains("passport-scan"),
            "a provider alias is understood"
        );
        let search = server
            .get("/admin/ai-api-logs?q=rate+limit")
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(search.contains("passport-scan") && !search.contains("translate-page"));

        let show = server
            .get(&format!("/admin/ai-api-logs/{}", bad.id))
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(
            show.contains(r#"data-testid="error""#)
                && show.contains("rate limit reached")
                && show.contains("prompt for passport-scan")
        );
        assert_eq!(
            server
                .get("/admin/ai-api-logs/999999")
                .add_header("cookie", cookie)
                .await
                .status_code(),
            404
        );

        let biz = login(&server, "biz@example.com", "password123").await;
        assert_eq!(
            server
                .get("/admin/ai-api-logs")
                .add_header("cookie", biz)
                .await
                .status_code(),
            403
        );
        assert_eq!(
            server.get("/admin/ai-api-logs").await.status_code(),
            303,
            "guests go to login"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn selected_or_matching_ai_api_logs_are_deleted_after_the_checks() {
    request::<App, _, _>(|server, ctx| async move {
        user(&ctx, "boss@example.com", Some("developer")).await;
        user(&ctx, "emp@example.com", Some("employee")).await;
        let first = seed(&ctx, "claude", "success", "one", 0).await;
        let second = seed(&ctx, "claude", "success", "two", 0).await;
        let third = seed(&ctx, "openai", "failed", "three", 0).await;
        let fourth = seed(&ctx, "openai", "failed", "four", 0).await;
        let boss = login(&server, "boss@example.com", "password123").await;
        let emp = login(&server, "emp@example.com", "password123").await;

        // An employee may read but not delete; nothing changes after the refusal.
        server
            .get("/admin/ai-api-logs")
            .add_header("cookie", emp.clone())
            .await
            .assert_status_ok();
        let refused = post(&server, "/admin/ai-api-logs/delete", &emp)
            .form(&[("scope", "selected"), ("ids", &first.id.to_string())])
            .await;
        assert_eq!(refused.status_code(), 403);
        assert_eq!(count(&ctx).await, 4);
        let page = server
            .get("/admin/ai-api-logs")
            .add_header("cookie", emp)
            .await
            .text();
        assert!(
            !page.contains("ai-api-logs-delete-selected"),
            "controls are hidden"
        );

        // A forged origin is refused.
        let forged = server
            .post("/admin/ai-api-logs/delete")
            .add_header("host", "localhost")
            .add_header("origin", "http://evil.example")
            .add_header("cookie", boss.clone())
            .form(&[("scope", "selected"), ("ids", &first.id.to_string())])
            .await;
        assert_eq!(forged.status_code(), 403);
        assert_eq!(count(&ctx).await, 4);

        // Delete selected, with the filters carried back to the list.
        let none = post(&server, "/admin/ai-api-logs/delete", &boss)
            .form(&[("scope", "selected"), ("return_to", "per_page=500")])
            .await;
        assert_eq!(none.status_code(), 303);
        assert_eq!(
            none.header("location").to_str().unwrap(),
            "/admin/ai-api-logs?error=none&per_page=500"
        );
        assert_eq!(count(&ctx).await, 4, "nothing selected, nothing deleted");

        let selected = post(&server, "/admin/ai-api-logs/delete", &boss)
            .form(&[
                ("scope", "selected"),
                ("ids", &first.id.to_string()),
                ("ids", &second.id.to_string()),
            ])
            .await;
        assert_eq!(
            selected.header("location").to_str().unwrap(),
            "/admin/ai-api-logs?saved=deleted&n=2"
        );
        assert_eq!(count(&ctx).await, 2);
        let banner = server
            .get("/admin/ai-api-logs?saved=deleted&n=2")
            .add_header("cookie", boss.clone())
            .await
            .text();
        assert!(banner.contains("2 AI API log(s) deleted."));

        // Delete everything that matches the filters, on every page.
        let matching = post(&server, "/admin/ai-api-logs/delete", &boss)
            .form(&[
                ("scope", "matching"),
                ("status", "failed"),
                ("provider", "openai"),
            ])
            .await;
        assert_eq!(
            matching.header("location").to_str().unwrap(),
            "/admin/ai-api-logs?saved=deleted&n=2"
        );
        assert_eq!(count(&ctx).await, 0);
        assert!(ai_api_logs::Entity::find_by_id(third.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none());
        assert!(ai_api_logs::Entity::find_by_id(fourth.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn one_row_and_old_rows_can_be_deleted_and_recent_ones_stay() {
    request::<App, _, _>(|server, ctx| async move {
        user(&ctx, "boss@example.com", Some("developer")).await;
        let boss = login(&server, "boss@example.com", "password123").await;
        let ancient = seed(&ctx, "claude", "success", "old", 90).await;
        let recent = seed(&ctx, "claude", "success", "new", 0).await;
        let single = seed(&ctx, "gemini", "success", "single", 0).await;

        let bad = post(&server, "/admin/ai-api-logs/purge", &boss)
            .form(&[("days", "0")])
            .await;
        assert_eq!(
            bad.header("location").to_str().unwrap(),
            "/admin/ai-api-logs?error=number"
        );
        assert_eq!(count(&ctx).await, 3);

        let purged = post(&server, "/admin/ai-api-logs/purge", &boss)
            .form(&[("days", "30")])
            .await;
        assert_eq!(
            purged.header("location").to_str().unwrap(),
            "/admin/ai-api-logs?saved=purged&n=1"
        );
        assert!(ai_api_logs::Entity::find_by_id(ancient.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none());
        assert!(ai_api_logs::Entity::find_by_id(recent.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some());

        let one = post(
            &server,
            &format!("/admin/ai-api-logs/{}/delete", single.id),
            &boss,
        )
        .await;
        assert_eq!(one.status_code(), 303);
        assert_eq!(count(&ctx).await, 1);
        assert_eq!(
            post(
                &server,
                &format!("/admin/ai-api-logs/{}/delete", single.id),
                &boss
            )
            .await
            .status_code(),
            404
        );
    })
    .await;
}
