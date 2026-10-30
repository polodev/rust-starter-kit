use loco_rs::{app::AppContext, boot::run_task, task, testing::prelude::*, TestServer};
use module_queue_watcher::{failed_jobs, jobs, queue_jobs, JobInfo};
use rust_starter_pack_helpers::*;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryOrder, Set};
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

async fn only_job(ctx: &AppContext) -> queue_jobs::Model {
    queue_jobs::Entity::find()
        .order_by_desc(queue_jobs::Column::Id)
        .one(&ctx.db)
        .await
        .unwrap()
        .expect("a tracked job")
}

#[tokio::test]
#[serial]
async fn track_records_success_failure_and_the_queued_flow() {
    let ctx = boot_test::<App>().await.unwrap().app_context;

    // A job that works: running while it runs, finished after.
    let value = module_queue_watcher::track(
        &ctx,
        JobInfo::new("SendDigest")
            .queue("mail")
            .payload(serde_json::json!({"to": 5})),
        async { Ok(42) },
    )
    .await
    .unwrap();
    assert_eq!(value, 42, "the result is returned unchanged");
    let job = only_job(&ctx).await;
    assert_eq!(
        (
            job.status.as_str(),
            job.attempt,
            job.queue.as_deref(),
            job.name.as_deref()
        ),
        ("finished", 1, Some("mail"), Some("SendDigest"))
    );
    assert!(
        job.started_at.is_some() && job.finished_at.is_some() && job.exception_message.is_none()
    );
    assert_eq!(job.payload.as_deref(), Some(r#"{"to":5}"#));

    // A job that fails: the error is recorded and still returned.
    let error = module_queue_watcher::track(&ctx, JobInfo::new("Broken"), async {
        Err::<(), _>(loco_rs::Error::string("disk full"))
    })
    .await
    .unwrap_err();
    assert!(error.to_string().contains("disk full"));
    let job = only_job(&ctx).await;
    assert_eq!(job.status, "failed");
    assert!(
        job.failed_at.is_some()
            && job
                .exception_message
                .as_deref()
                .unwrap()
                .contains("disk full")
    );
    assert!(job.exception_trace.is_some());

    // queued first, then picked up by a worker.
    let id = module_queue_watcher::queued(&ctx, &JobInfo::new("Later"))
        .await
        .unwrap();
    assert_eq!(only_job(&ctx).await.status, "queued");
    module_queue_watcher::track_queued(&ctx, &id, async { Ok(()) })
        .await
        .unwrap();
    let job = only_job(&ctx).await;
    assert_eq!(
        (job.status.as_str(), job.attempt, job.job_id.as_str()),
        ("finished", 1, id.as_str())
    );

    let stats = module_queue_watcher::stats(&ctx.db).await.unwrap();
    assert_eq!(
        (stats.finished, stats.failed, stats.queued, stats.running),
        (2, 1, 0, 0)
    );
}

#[tokio::test]
#[serial]
async fn the_app_worker_is_tracked_and_stale_and_purge_work() {
    use loco_rs::prelude::BackgroundWorker;
    let ctx = boot_test::<App>().await.unwrap().app_context;
    rust_starter::workers::downloader::DownloadWorker::build(&ctx)
        .perform(rust_starter::workers::downloader::DownloadWorkerArgs {
            user_guid: "abc".into(),
        })
        .await
        .unwrap();
    let job = only_job(&ctx).await;
    assert_eq!(
        (
            job.name.as_deref(),
            job.queue.as_deref(),
            job.status.as_str()
        ),
        (Some("DownloadWorker"), Some("downloads"), "finished")
    );

    let long_ago = chrono::Utc::now().fixed_offset() - chrono::Duration::days(30);
    let two_hours_ago = chrono::Utc::now().fixed_offset() - chrono::Duration::hours(2);
    let make = |name: &'static str, status: &'static str, at| {
        let ctx = ctx.clone();
        async move {
            queue_jobs::ActiveModel {
                job_id: Set(uuid::Uuid::new_v4().to_string()),
                name: Set(Some(name.into())),
                status: Set(status.into()),
                started_at: Set(Some(at)),
                attempt: Set(1),
                created_at: Set(Some(at)),
                ..Default::default()
            }
            .insert(&ctx.db)
            .await
            .unwrap()
        }
    };
    let hung = make("Hung", "running", two_hours_ago).await;
    let old_done = make("OldDone", "finished", long_ago).await;
    let old_running = make("OldRunning", "running", long_ago).await;

    let name = "queue-watcher:mark-stale".to_string();
    run_task::<App>(&ctx, Some(&name), &task::Vars::from_cli_args(vec![]))
        .await
        .unwrap();
    for id in [hung.id, old_running.id] {
        assert_eq!(
            queue_jobs::Entity::find_by_id(id)
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap()
                .status,
            "stale"
        );
    }
    let name = "queue-watcher:purge".to_string();
    run_task::<App>(
        &ctx,
        Some(&name),
        &task::Vars::from_cli_args(vec![("days".into(), "14".into())]),
    )
    .await
    .unwrap();
    assert!(
        queue_jobs::Entity::find_by_id(old_done.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none(),
        "old finished rows are purged"
    );
    assert!(
        queue_jobs::Entity::find_by_id(hung.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_some(),
        "recent rows stay"
    );
    assert!(run_task::<App>(
        &ctx,
        Some(&name),
        &task::Vars::from_cli_args(vec![("days".into(), "0".into())])
    )
    .await
    .is_err());
}

#[tokio::test]
#[serial]
async fn the_admin_screens_list_filter_show_and_delete() {
    request::<App, _, _>(|server, ctx| async move {
        user(&ctx, "boss@example.com", Some("developer")).await;
        user(&ctx, "emp@example.com", Some("employee")).await;
        user(&ctx, "biz@example.com", Some("business_development")).await;
        let now = chrono::Utc::now().fixed_offset();
        let make = |name: &'static str,
                    status: &'static str,
                    queue: &'static str,
                    error: Option<&'static str>| {
            let ctx = ctx.clone();
            async move {
                queue_jobs::ActiveModel {
                    job_id: Set(uuid::Uuid::new_v4().to_string()),
                    name: Set(Some(name.into())),
                    queue: Set(Some(queue.into())),
                    status: Set(status.into()),
                    attempt: Set(1),
                    started_at: Set(Some(now)),
                    finished_at: Set((status == "finished").then_some(now)),
                    failed_at: Set((status == "failed").then_some(now)),
                    exception_message: Set(error.map(Into::into)),
                    payload: Set(Some(r#"{"a":1}"#.into())),
                    created_at: Set(Some(now)),
                    ..Default::default()
                }
                .insert(&ctx.db)
                .await
                .unwrap()
            }
        };
        let good = make("Resize", "finished", "images", None).await;
        let bad = make("Export", "failed", "reports", Some("connection reset")).await;

        let boss = login(&server, "boss@example.com", "password123").await;
        let overview = server
            .get("/admin/queue-watcher")
            .add_header("cookie", boss.clone())
            .await;
        overview.assert_status_ok();
        let html = overview.text();
        assert!(
            html.contains(r#"data-testid="qw-tabs""#)
                && html.contains(r#"data-testid="breadcrumb""#)
        );
        assert!(
            html.contains(r#"data-testid="stat-finished">1<"#)
                && html.contains(r#"data-testid="stat-failed">1<"#)
        );

        let list = server
            .get("/admin/queue-watcher/jobs")
            .add_header("cookie", boss.clone())
            .await
            .text();
        assert_eq!(list.matches(r#"data-testid="id-link""#).count(), 2);
        assert!(list.contains("ID</th>") && list.contains("connection reset"));
        for (query, expected) in [
            ("status=failed", 1),
            ("status=finished", 1),
            ("queue=images", 1),
            ("q=Export", 1),
            ("q=nothing", 0),
            ("status=bogus", 2),
        ] {
            let html = server
                .get(&format!("/admin/queue-watcher/jobs?{query}"))
                .add_header("cookie", boss.clone())
                .await
                .text();
            assert_eq!(
                html.matches(r#"data-testid="id-link""#).count(),
                expected,
                "{query}"
            );
        }
        let show = server
            .get(&format!("/admin/queue-watcher/jobs/{}", bad.id))
            .add_header("cookie", boss.clone())
            .await
            .text();
        assert!(
            show.contains("Export")
                && show.contains("connection reset")
                && show.contains(r#"data-testid="job-error""#)
        );
        assert_eq!(
            server
                .get("/admin/queue-watcher/jobs/999999")
                .add_header("cookie", boss.clone())
                .await
                .status_code(),
            404
        );

        // Employees may look (queue.view) but not delete (queue.manage); others see nothing.
        let emp = login(&server, "emp@example.com", "password123").await;
        server
            .get("/admin/queue-watcher/jobs")
            .add_header("cookie", emp.clone())
            .await
            .assert_status_ok();
        let emp_list = server
            .get("/admin/queue-watcher/jobs")
            .add_header("cookie", emp.clone())
            .await
            .text();
        assert!(!emp_list.contains("delete-job-"));
        assert_eq!(
            post(
                &server,
                &format!("/admin/queue-watcher/jobs/{}/delete", good.id),
                &emp
            )
            .await
            .status_code(),
            403
        );
        let biz = login(&server, "biz@example.com", "password123").await;
        assert_eq!(
            server
                .get("/admin/queue-watcher")
                .add_header("cookie", biz)
                .await
                .status_code(),
            403
        );
        assert_eq!(server.get("/admin/queue-watcher").await.status_code(), 303);

        // Staff who can manage delete one record, purge, and mark stale.
        post(
            &server,
            &format!("/admin/queue-watcher/jobs/{}/delete", good.id),
            &boss,
        )
        .await;
        assert!(queue_jobs::Entity::find_by_id(good.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            post(
                &server,
                &format!("/admin/queue-watcher/jobs/{}/delete", good.id),
                &boss
            )
            .await
            .status_code(),
            404
        );
        let bad_days = post(&server, "/admin/queue-watcher/jobs/purge", &boss)
            .form(&[("days", "0")])
            .await;
        assert!(bad_days
            .header("location")
            .to_str()
            .unwrap()
            .contains("error=number"));
        let purged = post(&server, "/admin/queue-watcher/jobs/purge", &boss)
            .form(&[("days", "14")])
            .await;
        assert!(purged
            .header("location")
            .to_str()
            .unwrap()
            .contains("saved=purged&n=0"));
        let stale = post(&server, "/admin/queue-watcher/jobs/mark-stale", &boss)
            .form(&[("hours", "1")])
            .await;
        assert!(stale
            .header("location")
            .to_str()
            .unwrap()
            .contains("saved=stale"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn pending_and_failed_tables_of_a_laravel_queue_can_be_read_and_deleted() {
    request::<App, _, _>(|server, ctx| async move {
        user(&ctx, "boss@example.com", Some("developer")).await;
        let boss = login(&server, "boss@example.com", "password123").await;
        let payload = r#"{"uuid":"1","displayName":"App\\Jobs\\SendInvoice","job":"Illuminate\\Queue\\CallQueuedHandler@call","maxTries":3}"#;
        ctx.db
            .execute_unprepared(&format!(
                "INSERT INTO jobs (queue, payload, attempts, reserved_at, available_at, created_at) VALUES
                   ('emails', '{p}', 0, NULL, 1760000000, 1760000000),
                   ('emails', '{p}', 2, 1760000100, 1760000050, 1760000000);
                 INSERT INTO failed_jobs (uuid, connection, queue, payload, exception) VALUES
                   ('11111111-1111-4111-8111-111111111111', 'database', 'emails', '{p}', 'ErrorException: boom in /app/Jobs/SendInvoice.php:12\nStack trace:'),
                   ('22222222-2222-4222-8222-222222222222', 'database', 'default', '{p}', 'RuntimeException: other');",
                p = payload.replace('\'', "''").replace('\\', "\\\\")
            ))
            .await
            .unwrap();
        assert_eq!(jobs::Entity::find().count(&ctx.db).await.unwrap(), 2);
        assert_eq!(failed_jobs::Entity::find().count(&ctx.db).await.unwrap(), 2);

        let pending = server.get("/admin/queue-watcher/pending").add_header("cookie", boss.clone()).await.text();
        assert_eq!(pending.matches(r#"data-testid="id-link""#).count(), 2);
        assert!(pending.contains("SendInvoice") && pending.contains("emails") && pending.contains("2025-"));
        let first = jobs::Entity::find().order_by_asc(jobs::Column::Id).one(&ctx.db).await.unwrap().unwrap();
        let shown = server.get(&format!("/admin/queue-watcher/pending/{}", first.id)).add_header("cookie", boss.clone()).await.text();
        assert!(shown.contains(r#"data-testid="payload""#) && shown.contains("maxTries"));
        post(&server, &format!("/admin/queue-watcher/pending/{}/delete", first.id), &boss).await;
        assert_eq!(jobs::Entity::find().count(&ctx.db).await.unwrap(), 1);

        let failed = server.get("/admin/queue-watcher/failed").add_header("cookie", boss.clone()).await.text();
        assert_eq!(failed.matches(r#"data-testid="id-link""#).count(), 2);
        assert!(failed.contains("ErrorException: boom"));
        let narrowed = server.get("/admin/queue-watcher/failed?q=other").add_header("cookie", boss.clone()).await.text();
        assert_eq!(narrowed.matches(r#"data-testid="id-link""#).count(), 1);
        let one = failed_jobs::Entity::find().order_by_asc(failed_jobs::Column::Id).one(&ctx.db).await.unwrap().unwrap();
        let shown = server.get(&format!("/admin/queue-watcher/failed/{}", one.id)).add_header("cookie", boss.clone()).await.text();
        assert!(shown.contains(r#"data-testid="exception""#) && shown.contains("SendInvoice.php"));
        post(&server, &format!("/admin/queue-watcher/failed/{}/delete", one.id), &boss).await;
        assert_eq!(failed_jobs::Entity::find().count(&ctx.db).await.unwrap(), 1);
        let flushed = post(&server, "/admin/queue-watcher/failed/flush", &boss).await;
        assert!(flushed.header("location").to_str().unwrap().contains("saved=flushed&n=1"));
        assert_eq!(failed_jobs::Entity::find().count(&ctx.db).await.unwrap(), 0);
    })
    .await;
}
