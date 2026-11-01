use loco_rs::testing::prelude::*;
use module_queue_watcher::failed_jobs;
use sea_orm::{ActiveModelTrait, EntityTrait, QueryFilter, Set};
use serial_test::serial;

use rust_starter::app::App;

/// Search boxes find text whatever its case, on MySQL (case-insensitive collation) and PostgreSQL (`ILIKE`).
#[tokio::test]
#[serial]
async fn search_ignores_case_on_both_databases() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    failed_jobs::Entity::delete_many()
        .exec(&ctx.db)
        .await
        .unwrap();
    for (uuid, queue) in [("a-1", "Emails"), ("a-2", "reports")] {
        failed_jobs::ActiveModel {
            uuid: Set(uuid.to_string()),
            connection: Set("database".to_string()),
            queue: Set(queue.to_string()),
            payload: Set("{}".to_string()),
            exception: Set("Boom".to_string()),
            failed_at: Set(chrono::Utc::now().fixed_offset()),
            ..Default::default()
        }
        .insert(&ctx.db)
        .await
        .unwrap();
    }
    let found = |text: &str| {
        let ctx = ctx.clone();
        let text = text.to_string();
        async move {
            failed_jobs::Entity::find()
                .filter(loco_database::contains_ignoring_case(
                    failed_jobs::Column::Queue,
                    &text,
                ))
                .all(&ctx.db)
                .await
                .unwrap()
                .len()
        }
    };
    assert_eq!(found("emails").await, 1, "lower case finds Emails");
    assert_eq!(found("EMAIL").await, 1, "upper case finds Emails");
    assert_eq!(found("ort").await, 1, "a part of a word");
    assert_eq!(found("nothing").await, 0);
    failed_jobs::Entity::delete_many()
        .exec(&ctx.db)
        .await
        .unwrap();
}
