use std::{collections::HashMap, sync::Mutex};

use async_trait::async_trait;
use loco_database::{big_int, int, to_u32, to_u64};
use loco_modular::DatabaseJobHandler;
use loco_rs::{app::AppContext, testing::prelude::*, Error, Result};
use module_queue_watcher::{
    database_job, dispatch, failed_jobs, flush_failed, jobs, queue_jobs, retry_failed, work_once,
    DatabaseJob,
};
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serial_test::serial;

use rust_starter::app::App;

static RAN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

#[derive(Serialize, Deserialize)]
struct SendDigest {
    user_id: u32,
}

#[async_trait]
impl DatabaseJob for SendDigest {
    const NAME: &'static str = "TestSendDigest";
    fn queue() -> &'static str {
        "emails"
    }
    async fn handle(self, _ctx: &AppContext) -> Result<()> {
        RAN.lock().unwrap().push(self.user_id);
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct AlwaysFails;

#[async_trait]
impl DatabaseJob for AlwaysFails {
    const NAME: &'static str = "TestAlwaysFails";
    fn tries() -> u8 {
        2
    }
    async fn handle(self, _ctx: &AppContext) -> Result<()> {
        Err(Error::string("the mail server said no"))
    }
}

fn handlers() -> HashMap<&'static str, DatabaseJobHandler> {
    [database_job::<SendDigest>(), database_job::<AlwaysFails>()]
        .into_iter()
        .collect()
}

fn queues(names: &[&str]) -> Vec<String> {
    names.iter().map(ToString::to_string).collect()
}

async fn fresh() -> AppContext {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    jobs::Entity::delete_many().exec(&ctx.db).await.unwrap();
    failed_jobs::Entity::delete_many()
        .exec(&ctx.db)
        .await
        .unwrap();
    queue_jobs::Entity::delete_many()
        .exec(&ctx.db)
        .await
        .unwrap();
    RAN.lock().unwrap().clear();
    ctx
}

async fn make_due(ctx: &AppContext) {
    for row in jobs::Entity::find().all(&ctx.db).await.unwrap() {
        let mut due: jobs::ActiveModel = row.into();
        due.available_at = Set(0);
        due.update(&ctx.db).await.unwrap();
    }
}

#[tokio::test]
#[serial]
async fn a_dispatched_job_is_stored_in_laravels_jobs_table_and_the_worker_runs_it() {
    let ctx = fresh().await;
    let id = dispatch(SendDigest { user_id: 7 })
        .send(&ctx.db)
        .await
        .unwrap();

    let row = jobs::Entity::find_by_id(big_int(id))
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (row.queue.as_str(), row.attempts, row.reserved_at),
        ("emails", 0, None)
    );
    let payload: Value = serde_json::from_str(&row.payload).unwrap();
    assert_eq!(payload["displayName"], "TestSendDigest");
    assert_eq!(payload["maxTries"], 3);
    assert!(payload["uuid"]
        .as_str()
        .is_some_and(|uuid| uuid.len() == 36));
    assert_eq!(payload["data"]["commandName"], "TestSendDigest");
    assert_eq!(payload["data"]["command"], r#"{"user_id":7}"#);
    assert_eq!(payload["origin"], "rust-starter");

    assert_eq!(
        work_once(&ctx, &handlers(), &queues(&["emails"]))
            .await
            .unwrap(),
        1
    );
    assert_eq!(*RAN.lock().unwrap(), vec![7]);
    assert_eq!(jobs::Entity::find().count(&ctx.db).await.unwrap(), 0);
    let history = queue_jobs::Entity::find().all(&ctx.db).await.unwrap();
    assert_eq!(history.len(), 1, "the run shows in the queue monitor");
    assert_eq!(
        (history[0].status.as_str(), history[0].queue.as_deref()),
        ("finished", Some("emails"))
    );
    assert_eq!(
        work_once(&ctx, &handlers(), &queues(&["emails"]))
            .await
            .unwrap(),
        0,
        "nothing left"
    );
}

#[tokio::test]
#[serial]
async fn a_worker_runs_only_its_queues_and_only_jobs_that_are_due() {
    let ctx = fresh().await;
    dispatch(SendDigest { user_id: 1 })
        .send(&ctx.db)
        .await
        .unwrap();
    dispatch(SendDigest { user_id: 2 })
        .on_queue("reports")
        .send(&ctx.db)
        .await
        .unwrap();
    dispatch(SendDigest { user_id: 3 })
        .delay(3600)
        .send(&ctx.db)
        .await
        .unwrap();

    assert_eq!(
        work_once(&ctx, &handlers(), &queues(&["default"]))
            .await
            .unwrap(),
        0,
        "nothing on default"
    );
    assert_eq!(
        work_once(&ctx, &handlers(), &queues(&["reports", "emails"]))
            .await
            .unwrap(),
        2,
        "the delayed one waits"
    );
    let mut ran = RAN.lock().unwrap().clone();
    ran.sort_unstable();
    assert_eq!(ran, vec![1, 2]);
    assert_eq!(
        jobs::Entity::find().count(&ctx.db).await.unwrap(),
        1,
        "the delayed job is still stored"
    );
}

#[tokio::test]
#[serial]
async fn a_failing_job_is_tried_again_then_lands_in_failed_jobs_and_can_be_retried() {
    let ctx = fresh().await;
    dispatch(AlwaysFails).send(&ctx.db).await.unwrap();

    assert_eq!(
        work_once(&ctx, &handlers(), &queues(&["default"]))
            .await
            .unwrap(),
        1
    );
    let waiting = jobs::Entity::find().one(&ctx.db).await.unwrap().unwrap();
    assert_eq!(waiting.attempts, 1);
    assert!(waiting.reserved_at.is_none(), "released for the next try");
    assert!(waiting.available_at > waiting.created_at, "waits a while");
    assert_eq!(
        work_once(&ctx, &handlers(), &queues(&["default"]))
            .await
            .unwrap(),
        0,
        "not due yet"
    );

    make_due(&ctx).await;
    assert_eq!(
        work_once(&ctx, &handlers(), &queues(&["default"]))
            .await
            .unwrap(),
        1
    );
    assert_eq!(jobs::Entity::find().count(&ctx.db).await.unwrap(), 0);
    let failed = failed_jobs::Entity::find().all(&ctx.db).await.unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(
        (failed[0].connection.as_str(), failed[0].queue.as_str()),
        ("database", "default")
    );
    assert!(failed[0].exception.contains("the mail server said no"));
    assert_eq!(failed[0].uuid.len(), 36);

    assert_eq!(retry_failed(&ctx.db, "all").await.unwrap(), 1);
    assert_eq!(failed_jobs::Entity::find().count(&ctx.db).await.unwrap(), 0);
    let again = jobs::Entity::find().one(&ctx.db).await.unwrap().unwrap();
    assert_eq!(
        (again.attempts, again.queue.as_str(), again.payload.as_str()),
        (0, "default", failed[0].payload.as_str())
    );

    make_due(&ctx).await;
    work_once(&ctx, &handlers(), &queues(&["default"]))
        .await
        .unwrap();
    make_due(&ctx).await;
    work_once(&ctx, &handlers(), &queues(&["default"]))
        .await
        .unwrap();
    assert_eq!(flush_failed(&ctx.db).await.unwrap(), 1);
    assert_eq!(failed_jobs::Entity::find().count(&ctx.db).await.unwrap(), 0);
}

#[tokio::test]
#[serial]
async fn a_job_from_a_laravel_app_is_left_alone() {
    let ctx = fresh().await;
    let laravel_payload = r#"{"uuid":"6f8a0f1e-0000-4000-8000-000000000001","displayName":"App\\Jobs\\SendInvoice","job":"Illuminate\\Queue\\CallQueuedHandler@call","maxTries":3,"data":{"commandName":"App\\Jobs\\SendInvoice","command":"O:20:\"App\\Jobs\\SendInvoice\":0:{}"}}"#;
    jobs::ActiveModel {
        queue: Set("emails".to_string()),
        payload: Set(laravel_payload.to_string()),
        attempts: Set(0),
        available_at: Set(0),
        created_at: Set(0),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap();

    assert_eq!(
        work_once(&ctx, &handlers(), &queues(&["emails"]))
            .await
            .unwrap(),
        0
    );
    let row = jobs::Entity::find()
        .order_by_asc(jobs::Column::Id)
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((row.attempts, row.reserved_at), (0, None), "untouched");
}

#[tokio::test]
#[serial]
async fn two_workers_never_run_the_same_job_twice() {
    let ctx = fresh().await;
    for user_id in 0..8 {
        dispatch(SendDigest { user_id })
            .send(&ctx.db)
            .await
            .unwrap();
    }
    let handlers = handlers();
    let all = queues(&["emails"]);
    let (a, b) = tokio::join!(
        work_once(&ctx, &handlers, &all),
        work_once(&ctx, &handlers, &all)
    );
    assert_eq!(a.unwrap() + b.unwrap(), 8);
    let mut ran = RAN.lock().unwrap().clone();
    ran.sort_unstable();
    assert_eq!(
        ran,
        (0..8).collect::<Vec<u32>>(),
        "each job ran exactly once"
    );
}

// ---- harder cases ----

#[derive(Serialize, Deserialize)]
struct Panics;

#[async_trait]
impl DatabaseJob for Panics {
    const NAME: &'static str = "TestPanics";
    fn tries() -> u8 {
        1
    }
    async fn handle(self, _ctx: &AppContext) -> Result<()> {
        panic!("boom");
    }
}

fn all_handlers() -> HashMap<&'static str, DatabaseJobHandler> {
    [
        database_job::<SendDigest>(),
        database_job::<AlwaysFails>(),
        database_job::<Panics>(),
    ]
    .into_iter()
    .collect()
}

async fn insert_raw(ctx: &AppContext, queue: &str, payload: &str) -> u64 {
    let id = jobs::ActiveModel {
        queue: Set(queue.to_string()),
        payload: Set(payload.to_string()),
        attempts: Set(0),
        available_at: Set(0),
        created_at: Set(0),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await
    .unwrap()
    .id;
    to_u64(id)
}

fn envelope(name: &str, command: &str, tries: u8) -> String {
    serde_json::json!({
        "uuid": uuid_like(name), "displayName": name, "maxTries": tries, "backoff": [60],
        "data": { "commandName": name, "command": command }, "origin": "rust-starter",
    })
    .to_string()
}

fn uuid_like(seed: &str) -> String {
    format!("00000000-0000-4000-8000-{:0>12}", seed.len())
}

#[tokio::test]
#[serial]
async fn a_panicking_job_fails_its_try_and_the_worker_keeps_going() {
    let ctx = fresh().await;
    dispatch(Panics).send(&ctx.db).await.unwrap();
    dispatch(SendDigest { user_id: 5 })
        .send(&ctx.db)
        .await
        .unwrap();
    let handled = work_once(&ctx, &all_handlers(), &queues(&["default", "emails"]))
        .await
        .unwrap();
    assert_eq!(handled, 2, "the pass finished despite the panic");
    assert_eq!(*RAN.lock().unwrap(), vec![5], "the next job still ran");
    let failed = failed_jobs::Entity::find().all(&ctx.db).await.unwrap();
    assert_eq!(failed.len(), 1);
    assert!(
        failed[0].exception.contains("panicked"),
        "{}",
        failed[0].exception
    );
}

#[tokio::test]
#[serial]
async fn broken_payloads_go_to_failed_jobs_instead_of_blocking_the_queue() {
    let ctx = fresh().await;
    // Marked as ours but not JSON, JSON without the fields, a name nobody registered, and bad arguments.
    insert_raw(&ctx, "default", "{\"origin\":\"rust-starter\" not json").await;
    insert_raw(&ctx, "default", "{\"origin\":\"rust-starter\"}").await;
    insert_raw(&ctx, "default", &envelope("NobodyKnowsMe", "{}", 1)).await;
    insert_raw(
        &ctx,
        "default",
        &envelope("TestSendDigest", "{\"wrong\":true}", 1),
    )
    .await;
    dispatch(SendDigest { user_id: 9 })
        .on_queue("default")
        .send(&ctx.db)
        .await
        .unwrap();

    assert_eq!(
        work_once(&ctx, &handlers(), &queues(&["default"]))
            .await
            .unwrap(),
        5
    );
    assert_eq!(
        *RAN.lock().unwrap(),
        vec![9],
        "the good job was not blocked"
    );
    assert_eq!(jobs::Entity::find().count(&ctx.db).await.unwrap(), 0);
    let failed = failed_jobs::Entity::find()
        .order_by_asc(failed_jobs::Column::Id)
        .all(&ctx.db)
        .await
        .unwrap();
    assert_eq!(failed.len(), 4);
    assert!(
        failed[0].exception.contains("not JSON"),
        "{}",
        failed[0].exception
    );
    assert!(
        failed[1].exception.contains("displayName"),
        "{}",
        failed[1].exception
    );
    assert!(
        failed[2].exception.contains("no handler is registered"),
        "{}",
        failed[2].exception
    );
    assert!(
        failed[3].exception.contains("bad arguments"),
        "{}",
        failed[3].exception
    );
    let uuids: std::collections::HashSet<_> = failed.iter().map(|row| row.uuid.clone()).collect();
    assert_eq!(uuids.len(), 4, "every failed job has its own uuid");
}

#[tokio::test]
#[serial]
async fn the_back_off_grows_and_tries_can_be_overridden() {
    let ctx = fresh().await;
    // Default AlwaysFails has 2 tries; a third try is never made.
    let before = chrono::Utc::now().timestamp() as u32;
    dispatch(AlwaysFails).tries(3).send(&ctx.db).await.unwrap();
    work_once(&ctx, &handlers(), &queues(&["default"]))
        .await
        .unwrap();
    let first = jobs::Entity::find().one(&ctx.db).await.unwrap().unwrap();
    assert!(
        (to_u32(first.available_at) - before) >= 59 && (to_u32(first.available_at) - before) <= 65,
        "first wait is about 60 s, got {}",
        to_u32(first.available_at) - before
    );
    make_due(&ctx).await;
    work_once(&ctx, &handlers(), &queues(&["default"]))
        .await
        .unwrap();
    let second = jobs::Entity::find().one(&ctx.db).await.unwrap().unwrap();
    assert_eq!(second.attempts, 2);
    assert!(
        (to_u32(second.available_at) - before) >= 299
            && (to_u32(second.available_at) - before) <= 306,
        "second wait is about 300 s"
    );

    // tries(1): straight to failed_jobs after one attempt.
    jobs::Entity::delete_many().exec(&ctx.db).await.unwrap();
    dispatch(AlwaysFails).tries(1).send(&ctx.db).await.unwrap();
    work_once(&ctx, &handlers(), &queues(&["default"]))
        .await
        .unwrap();
    assert_eq!(jobs::Entity::find().count(&ctx.db).await.unwrap(), 0);
    assert_eq!(failed_jobs::Entity::find().count(&ctx.db).await.unwrap(), 1);
}

#[tokio::test]
#[serial]
async fn dispatch_inside_a_transaction_exists_only_if_the_transaction_commits() {
    use sea_orm::TransactionTrait;
    let ctx = fresh().await;

    let rolled_back = ctx.db.begin().await.unwrap();
    dispatch(SendDigest { user_id: 1 })
        .send(&rolled_back)
        .await
        .unwrap();
    rolled_back.rollback().await.unwrap();
    assert_eq!(jobs::Entity::find().count(&ctx.db).await.unwrap(), 0);

    let committed = ctx.db.begin().await.unwrap();
    dispatch(SendDigest { user_id: 2 })
        .send(&committed)
        .await
        .unwrap();
    committed.commit().await.unwrap();
    assert_eq!(jobs::Entity::find().count(&ctx.db).await.unwrap(), 1);
}

#[tokio::test]
#[serial]
async fn a_job_held_by_a_crashed_worker_is_taken_over_but_a_live_hold_is_respected() {
    let ctx = fresh().await;
    let id = dispatch(SendDigest { user_id: 4 })
        .send(&ctx.db)
        .await
        .unwrap();
    let now = chrono::Utc::now().timestamp() as u32;

    let mut held: jobs::ActiveModel = jobs::Entity::find_by_id(big_int(id))
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap()
        .into();
    held.reserved_at = Set(Some(int(now)));
    held.attempts = Set(1);
    held.update(&ctx.db).await.unwrap();
    assert_eq!(
        work_once(&ctx, &handlers(), &queues(&["emails"]))
            .await
            .unwrap(),
        0,
        "a live worker holds it"
    );
    assert!(RAN.lock().unwrap().is_empty());

    let mut abandoned: jobs::ActiveModel = jobs::Entity::find_by_id(big_int(id))
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap()
        .into();
    abandoned.reserved_at = Set(Some(int(now - 3600)));
    abandoned.update(&ctx.db).await.unwrap();
    assert_eq!(
        work_once(&ctx, &handlers(), &queues(&["emails"]))
            .await
            .unwrap(),
        1
    );
    assert_eq!(*RAN.lock().unwrap(), vec![4]);
}

#[tokio::test]
#[serial]
async fn failed_jobs_can_be_retried_one_at_a_time_by_id_or_uuid() {
    let ctx = fresh().await;
    for _ in 0..3 {
        dispatch(AlwaysFails).tries(1).send(&ctx.db).await.unwrap();
    }
    work_once(&ctx, &handlers(), &queues(&["default"]))
        .await
        .unwrap();
    let failed = failed_jobs::Entity::find()
        .order_by_asc(failed_jobs::Column::Id)
        .all(&ctx.db)
        .await
        .unwrap();
    assert_eq!(failed.len(), 3);

    assert_eq!(
        retry_failed(&ctx.db, &failed[0].id.to_string())
            .await
            .unwrap(),
        1
    );
    assert_eq!(retry_failed(&ctx.db, &failed[1].uuid).await.unwrap(), 1);
    assert_eq!(retry_failed(&ctx.db, "no-such-uuid").await.unwrap(), 0);
    assert_eq!(retry_failed(&ctx.db, "999999").await.unwrap(), 0);
    assert_eq!(jobs::Entity::find().count(&ctx.db).await.unwrap(), 2);
    let left = failed_jobs::Entity::find().all(&ctx.db).await.unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id, failed[2].id, "the third one is untouched");
}

#[tokio::test]
#[serial]
async fn jobs_run_oldest_first_and_a_large_payload_survives() {
    let ctx = fresh().await;
    for user_id in [30, 10, 20] {
        dispatch(SendDigest { user_id })
            .send(&ctx.db)
            .await
            .unwrap();
    }
    work_once(&ctx, &handlers(), &queues(&["emails"]))
        .await
        .unwrap();
    assert_eq!(*RAN.lock().unwrap(), vec![30, 10, 20], "dispatch order");

    #[derive(Serialize, Deserialize)]
    struct Big {
        text: String,
    }
    #[async_trait]
    impl DatabaseJob for Big {
        const NAME: &'static str = "TestBig";
        async fn handle(self, _ctx: &AppContext) -> Result<()> {
            RAN.lock().unwrap().push(self.text.len() as u32);
            Ok(())
        }
    }
    RAN.lock().unwrap().clear();
    let text = "é".repeat(500_000); // about 1 MB of UTF-8
    dispatch(Big { text }).send(&ctx.db).await.unwrap();
    let all: HashMap<&'static str, DatabaseJobHandler> =
        [database_job::<Big>()].into_iter().collect();
    work_once(&ctx, &all, &queues(&["default"])).await.unwrap();
    assert_eq!(*RAN.lock().unwrap(), vec![1_000_000]);
}

#[tokio::test]
#[serial]
async fn several_workers_share_many_jobs_without_losing_or_repeating_any() {
    let ctx = fresh().await;
    for user_id in 0..60 {
        dispatch(SendDigest { user_id })
            .send(&ctx.db)
            .await
            .unwrap();
    }
    let handlers = handlers();
    let names = queues(&["emails"]);
    let mut total = 0;
    // Passes are limited to 20 jobs, so several rounds of three concurrent workers drain 60.
    for _ in 0..4 {
        let (a, b, c) = tokio::join!(
            work_once(&ctx, &handlers, &names),
            work_once(&ctx, &handlers, &names),
            work_once(&ctx, &handlers, &names)
        );
        total += a.unwrap() + b.unwrap() + c.unwrap();
    }
    assert_eq!(total, 60);
    let mut ran = RAN.lock().unwrap().clone();
    ran.sort_unstable();
    assert_eq!(ran, (0..60).collect::<Vec<u32>>(), "each job exactly once");
    assert_eq!(jobs::Entity::find().count(&ctx.db).await.unwrap(), 0);
}

#[tokio::test]
#[serial]
async fn the_jobs_work_command_runs_due_jobs_and_the_failed_commands_work() {
    use loco_rs::task::{Task, Vars};
    use module_queue_watcher::{JobsFailed, JobsFlushFailed, JobsRetryFailed, JobsWork};
    let ctx = fresh().await;
    dispatch(SendDigest { user_id: 12 })
        .send(&ctx.db)
        .await
        .unwrap();
    dispatch(AlwaysFails).tries(1).send(&ctx.db).await.unwrap();

    let work = JobsWork::new(all_handlers());
    let once = |queue: &str| {
        Vars::from_cli_args(vec![
            ("queue".to_string(), queue.to_string()),
            ("once".to_string(), "true".to_string()),
        ])
    };
    work.run(&ctx, &once("emails")).await.unwrap();
    assert_eq!(*RAN.lock().unwrap(), vec![12]);
    assert_eq!(
        jobs::Entity::find().count(&ctx.db).await.unwrap(),
        1,
        "the default queue was not touched"
    );
    work.run(&ctx, &once("default")).await.unwrap();
    assert_eq!(failed_jobs::Entity::find().count(&ctx.db).await.unwrap(), 1);

    JobsFailed.run(&ctx, &Vars::default()).await.unwrap();
    assert!(
        JobsRetryFailed.run(&ctx, &Vars::default()).await.is_err(),
        "needs which:"
    );
    JobsRetryFailed
        .run(
            &ctx,
            &Vars::from_cli_args(vec![("which".to_string(), "all".to_string())]),
        )
        .await
        .unwrap();
    assert_eq!(failed_jobs::Entity::find().count(&ctx.db).await.unwrap(), 0);
    assert_eq!(jobs::Entity::find().count(&ctx.db).await.unwrap(), 1);
    work.run(&ctx, &once("default")).await.unwrap();
    JobsFlushFailed.run(&ctx, &Vars::default()).await.unwrap();
    assert_eq!(failed_jobs::Entity::find().count(&ctx.db).await.unwrap(), 0);
}
