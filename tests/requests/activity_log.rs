use loco_activity_log::activity_log;
use loco_rs::{app::AppContext, boot::run_task, task, testing::prelude::*, TestServer};
use rust_starter_pack_helpers::*;
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, QueryOrder, Set};
use serde_json::{json, Value};
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

type App = rust_starter::app::App;

async fn entries(ctx: &AppContext) -> Vec<activity_log::Model> {
    activity_log::Entity::find()
        .order_by_asc(activity_log::Column::Id)
        .all(&ctx.db)
        .await
        .unwrap()
}

async fn last(ctx: &AppContext) -> activity_log::Model {
    entries(ctx).await.pop().expect("an activity entry")
}

#[tokio::test]
#[serial]
async fn real_actions_write_spatie_shaped_entries() {
    request::<App, _, _>(|server, ctx| async move {
        user(&ctx, "boss@example.com", Some("developer")).await;
        user(&ctx, "worker@example.com", None).await;
        let cookie = login(&server, "boss@example.com", "password123").await;
        let boss = rust_starter::models::users::Model::find_by_email(&ctx.db, "boss@example.com")
            .await
            .unwrap();
        let worker =
            rust_starter::models::users::Model::find_by_email(&ctx.db, "worker@example.com")
                .await
                .unwrap();

        // Signing in is logged, with who did it.
        let login_entry = entries(&ctx)
            .await
            .into_iter()
            .find(|e| e.event.as_deref() == Some("login"))
            .unwrap();
        assert_eq!(login_entry.log_name.as_deref(), Some("auth"));
        assert_eq!(login_entry.causer_type.as_deref(), Some("user"));
        assert_eq!(
            login_entry.causer_id.as_deref(),
            Some(boss.id.to_string().as_str())
        );

        // Editing a user logs only what changed, as `attributes` and `old`.
        post(&server, &format!("/admin/users/{}", worker.id), &cookie)
            .form(&[("name", "Worker Renamed"), ("email", "worker@example.com")])
            .await;
        let edit = last(&ctx).await;
        assert_eq!(
            (
                edit.log_name.as_deref(),
                edit.event.as_deref(),
                edit.subject_type.as_deref()
            ),
            (Some("users"), Some("updated"), Some("user"))
        );
        assert_eq!(
            edit.subject_id.as_deref(),
            Some(worker.id.to_string().as_str())
        );
        assert_eq!(
            edit.causer_id.as_deref(),
            Some(boss.id.to_string().as_str())
        );
        assert_eq!(
            edit.properties_json(),
            json!({"attributes": {"name": "Worker Renamed"}, "old": {"name": "Test Person"}})
        );

        // A generated password is logged without the password.
        let generated = post(
            &server,
            &format!("/admin/users/{}/password", worker.id),
            &cookie,
        )
        .await
        .text();
        let marker = r#"data-testid="new-password">"#;
        let start = generated.find(marker).unwrap() + marker.len();
        let password = &generated[start..start + 16];
        let entry = last(&ctx).await;
        assert_eq!(entry.event.as_deref(), Some("password_generated"));
        assert!(
            !entry.description.contains(password)
                && !entry
                    .properties
                    .clone()
                    .unwrap_or_default()
                    .contains(password)
        );

        // A role change logs the role before and after.
        post(
            &server,
            &format!("/admin/users/{}/role", worker.id),
            &cookie,
        )
        .form(&[("role", "marketing")])
        .await;
        let entry = last(&ctx).await;
        assert_eq!(entry.event.as_deref(), Some("role_changed"));
        assert_eq!(
            entry.properties_json(),
            json!({"attributes": {"role": "Marketing"}, "old": {"role": null}})
        );

        // A page being published or hidden, and a contact status change.
        module_role_permission::sync(&ctx.db).await.ok();
        let page = module_pages::pages::ActiveModel {
            slug: Set("log-demo".into()),
            is_published: Set(true),
            sort_order: Set(0),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
        post(
            &server,
            &format!("/admin/pages/{}/published", page.id),
            &cookie,
        )
        .form(&[("published", "0")])
        .await;
        let entry = last(&ctx).await;
        assert_eq!(
            (entry.event.as_deref(), entry.subject_type.as_deref()),
            (Some("unpublished"), Some("page"))
        );
        assert_eq!(
            entry.properties_json(),
            json!({"attributes": {"is_published": false}, "old": {"is_published": true}})
        );

        // Logging out is logged too.
        post(&server, "/logout", &cookie).await;
        assert_eq!(last(&ctx).await.event.as_deref(), Some("logout"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn the_admin_list_filters_shows_changes_and_needs_the_permission() {
    request::<App, _, _>(|server, ctx| async move {
        user(&ctx, "boss@example.com", Some("developer")).await;
        user(&ctx, "biz@example.com", Some("business_development")).await;
        let cookie = login(&server, "boss@example.com", "password123").await;

        // Written the way a Laravel app writes it (numeric id as text, extra properties).
        let written = activity_log::ActiveModel {
            log_name: Set(Some("orders".into())),
            description: Set("Order shipped".into()),
            subject_type: Set(Some("order".into())),
            subject_id: Set(Some("77".into())),
            event: Set(Some("updated".into())),
            causer_type: Set(Some("user".into())),
            causer_id: Set(Some("1".into())),
            properties: Set(Some(
                r#"{"attributes":{"status":"shipped"},"old":{"status":"packed"},"ip":"10.0.0.1"}"#
                    .into(),
            )),
            created_at: Set(Some(chrono::Utc::now().fixed_offset())),
            updated_at: Set(Some(chrono::Utc::now().fixed_offset())),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();

        let list = server
            .get("/admin/activity-log")
            .add_header("cookie", cookie.clone())
            .await;
        list.assert_status_ok();
        let html = list.text();
        assert!(
            html.contains("Order shipped")
                && html.contains(&format!(r#"href="/admin/activity-log/{}""#, written.id))
        );
        assert!(html.contains(r#"data-testid="breadcrumb""#) && html.contains("ID</th>"));

        for (query, expected) in [
            ("log=orders", 1),
            ("log=auth", 1),
            ("event=updated", 1),
            ("q=shipped", 1),
            ("q=nothing-like-this", 0),
        ] {
            let html = server
                .get(&format!("/admin/activity-log?{query}"))
                .add_header("cookie", cookie.clone())
                .await
                .text();
            assert_eq!(
                html.matches(r#"data-testid="id-link""#).count(),
                expected,
                "{query}"
            );
        }

        let show = server
            .get(&format!("/admin/activity-log/{}", written.id))
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(
            show.contains("Order shipped") && show.contains(r#"data-testid="activity-changes""#)
        );
        assert!(show.contains("packed") && show.contains("shipped") && show.contains("10.0.0.1"));
        assert_eq!(
            server
                .get("/admin/activity-log/999999")
                .add_header("cookie", cookie)
                .await
                .status_code(),
            404
        );

        // Business development has no activity_logs.view.
        let biz = login(&server, "biz@example.com", "password123").await;
        assert_eq!(
            server
                .get("/admin/activity-log")
                .add_header("cookie", biz)
                .await
                .status_code(),
            403
        );
        assert_eq!(
            server.get("/admin/activity-log").await.status_code(),
            303,
            "guests go to login"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn deleting_and_cleaning_need_manage_and_keep_recent_entries() {
    request::<App, _, _>(|server, ctx| async move {
        user(&ctx, "boss@example.com", Some("developer")).await;
        user(&ctx, "emp@example.com", Some("employee")).await;
        let old = chrono::Utc::now().fixed_offset() - chrono::Duration::days(400);
        let make = |name: &'static str, at| {
            let ctx = ctx.clone();
            async move {
                activity_log::ActiveModel {
                    log_name: Set(Some(name.into())),
                    description: Set(format!("entry in {name}")),
                    created_at: Set(Some(at)),
                    updated_at: Set(Some(at)),
                    ..Default::default()
                }
                .insert(&ctx.db)
                .await
                .unwrap()
            }
        };
        let ancient = make("old-log", old).await;
        let recent = make("new-log", chrono::Utc::now().fixed_offset()).await;

        // An employee can read the log (activity_logs.view) but not delete (activity_logs.manage).
        let emp = login(&server, "emp@example.com", "password123").await;
        server
            .get("/admin/activity-log")
            .add_header("cookie", emp.clone())
            .await
            .assert_status_ok();
        assert_eq!(
            post(
                &server,
                &format!("/admin/activity-log/{}/delete", recent.id),
                &emp
            )
            .await
            .status_code(),
            403
        );
        assert_eq!(
            post(&server, "/admin/activity-log/clean", &emp)
                .form(&[("days", "30")])
                .await
                .status_code(),
            403
        );

        let boss = login(&server, "boss@example.com", "password123").await;
        let bad = post(&server, "/admin/activity-log/clean", &boss)
            .form(&[("days", "0")])
            .await;
        assert!(bad
            .header("location")
            .to_str()
            .unwrap()
            .contains("error=days"));
        let cleaned = post(&server, "/admin/activity-log/clean", &boss)
            .form(&[("days", "365")])
            .await;
        assert!(cleaned
            .header("location")
            .to_str()
            .unwrap()
            .contains("saved=cleaned&n=1"));
        assert!(activity_log::Entity::find_by_id(ancient.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none());
        assert!(activity_log::Entity::find_by_id(recent.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some());

        post(
            &server,
            &format!("/admin/activity-log/{}/delete", recent.id),
            &boss,
        )
        .await;
        assert!(activity_log::Entity::find_by_id(recent.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            post(
                &server,
                &format!("/admin/activity-log/{}/delete", recent.id),
                &boss
            )
            .await
            .status_code(),
            404
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn the_clean_command_and_the_off_switch_work() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    let old = chrono::Utc::now().fixed_offset() - chrono::Duration::days(40);
    for (log, at) in [
        ("a", old),
        ("b", old),
        ("b", chrono::Utc::now().fixed_offset()),
    ] {
        activity_log::ActiveModel {
            log_name: Set(Some(log.into())),
            description: Set("x".into()),
            created_at: Set(Some(at)),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
    }
    let name = "activitylog:clean".to_string();
    let vars = |pairs: &[(&str, &str)]| {
        task::Vars::from_cli_args(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    };
    // Only log "a", only entries older than 30 days.
    run_task::<App>(&ctx, Some(&name), &vars(&[("days", "30"), ("log", "a")]))
        .await
        .unwrap();
    assert_eq!(
        activity_log::Entity::find().count(&ctx.db).await.unwrap(),
        2
    );
    run_task::<App>(&ctx, Some(&name), &vars(&[("days", "30")]))
        .await
        .unwrap();
    assert_eq!(
        activity_log::Entity::find().count(&ctx.db).await.unwrap(),
        1
    );
    assert!(
        run_task::<App>(&ctx, Some(&name), &vars(&[("days", "soon")]))
            .await
            .is_err()
    );

    // ACTIVITY_LOGGER_ENABLED=false switches logging off, like Spatie's config.
    std::env::set_var("ACTIVITY_LOGGER_ENABLED", "false");
    let skipped = loco_activity_log::activity(&ctx.db)
        .log("not written")
        .await
        .unwrap();
    std::env::remove_var("ACTIVITY_LOGGER_ENABLED");
    assert!(skipped.is_none());
    assert_eq!(
        activity_log::Entity::find().count(&ctx.db).await.unwrap(),
        1
    );

    let written = loco_activity_log::activity(&ctx.db)
        .in_log("demo")
        .performed_on("page", 5)
        .caused_by("user", 9)
        .event("created")
        .with_property("name", "Ada")
        .log("Created :properties.name")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(written.description, "Created Ada");
    assert_eq!(
        (written.subject_id.as_deref(), written.causer_id.as_deref()),
        (Some("5"), Some("9"))
    );
    let _: Value = written.properties_json();
}
