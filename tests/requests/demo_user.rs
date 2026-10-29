use loco_rs::testing::prelude::*;
use rust_starter::{
    app::App,
    models::users,
    seeders::demo_user::{self, DEMO_EMAIL, DEMO_PASSWORD},
};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn demo_user_is_created_verified_and_can_log_in() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();

        let user = users::Model::find_by_email(&ctx.db, DEMO_EMAIL)
            .await
            .unwrap();
        assert!(
            user.email_verified_at.is_some(),
            "demo user should be verified"
        );
        assert!(user.verify_password(DEMO_PASSWORD));
        assert!(!user.verify_password("wrong-password"));

        let response = server
            .post("/api/auth/login")
            .json(&serde_json::json!({ "email": DEMO_EMAIL, "password": DEMO_PASSWORD }))
            .await;
        response.assert_status_ok();
        assert!(response.text().contains("token"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn demo_user_seeder_is_idempotent() {
    request::<App, _, _>(|_server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        let first = users::Model::find_by_email(&ctx.db, DEMO_EMAIL)
            .await
            .unwrap();
        demo_user::run(&ctx.db).await.unwrap();
        let second = users::Model::find_by_email(&ctx.db, DEMO_EMAIL)
            .await
            .unwrap();
        assert_eq!(first.id, second.id);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn demo_user_is_not_seeded_outside_development() {
    request::<App, _, _>(|_server, ctx| async move {
        // The test environment must never get a user with a publicly known password.
        demo_user::run_in_development(&ctx).await.unwrap();
        assert!(users::Model::find_by_email(&ctx.db, DEMO_EMAIL)
            .await
            .is_err());
    })
    .await;
}
