use loco_rs::{app::Hooks, boot::run_task, environment::Environment, task, testing::prelude::*};
use rust_starter::app::App;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serial_test::serial;

fn vars(strict: bool) -> task::Vars {
    task::Vars::from_cli_args(vec![("strict".to_string(), strict.to_string())])
}

#[tokio::test]
#[serial]
async fn passes_when_every_page_is_translated() {
    let boot = boot_test::<App>().await.unwrap();
    loco_modular::seed_all(&modules_registry::all(), &boot.app_context)
        .await
        .unwrap();

    let name = "pages:check-translations".to_string();
    run_task::<App>(&boot.app_context, Some(&name), &vars(true))
        .await
        .unwrap();
}

#[tokio::test]
#[serial]
async fn strict_mode_fails_when_a_translation_is_missing() {
    let boot = boot_test::<App>().await.unwrap();
    let db = &boot.app_context.db;
    module_pages::pages::ActiveModel {
        slug: Set("untranslated".to_string()),
        is_published: Set(true),
        sort_order: Set(9),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();

    let name = "pages:check-translations".to_string();
    // Reporting alone succeeds, strict mode fails.
    run_task::<App>(&boot.app_context, Some(&name), &vars(false))
        .await
        .unwrap();
    let error = run_task::<App>(&boot.app_context, Some(&name), &vars(true))
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("2 translation(s) missing"),
        "got: {error}"
    );

    module_pages::pages::Entity::delete_many()
        .exec(db)
        .await
        .unwrap();
}

#[tokio::test]
async fn module_schedules_are_merged_into_the_scheduler_config() {
    let config = App::load_config(&Environment::Test).await.unwrap();
    let jobs = config.scheduler.expect("scheduler section").jobs;

    let job = jobs
        .get("pages-check-translations")
        .expect("pages job is scheduled");
    assert_eq!(job.run, "pages:check-translations");
    assert_eq!(job.cron, "0 0 3 * * *");
    assert_eq!(job.tags.as_deref(), Some(&["content".to_string()][..]));
}
