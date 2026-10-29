use loco_rs::testing::prelude::*;
use rust_starter::{
    app::App,
    seeders::demo_user::{self, DEMO_EMAIL, DEMO_PASSWORD},
};
use serde_json::{json, Value};
use serial_test::serial;

const BASE: &str = "/api/v1/auth";

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

#[tokio::test]
#[serial]
async fn register_login_user_and_logout_work_like_sanctum() {
    request::<App, _, _>(|server, _ctx| async move {
        let registered = server
            .post(&format!("{BASE}/register"))
            .json(&json!({
                "name": "Api Person",
                "email": "Api.Person@Example.com",
                "password": "correct horse",
                "password_confirmation": "correct horse",
                "device_name": "phone"
            }))
            .await;
        assert_eq!(registered.status_code(), 201, "{}", registered.text());
        let body: Value = registered.json();
        assert_eq!(body["user"]["email"], "api.person@example.com");
        assert!(body["user"].get("password").is_none());
        let first_token = body["token"].as_str().unwrap().to_string();
        assert!(first_token.contains('|'));

        let me: Value = server
            .get(&format!("{BASE}/user"))
            .add_header("authorization", bearer(&first_token))
            .await
            .json();
        assert_eq!(me["data"]["name"], "Api Person");

        let login = server
            .post(&format!("{BASE}/login"))
            .json(&json!({"email": "api.person@example.com", "password": "correct horse", "device_name": "laptop"}))
            .await;
        login.assert_status_ok();
        let second_token = login.json::<Value>()["token"].as_str().unwrap().to_string();
        assert_ne!(first_token, second_token);

        let listed: Value = server
            .get(&format!("{BASE}/tokens"))
            .add_header("authorization", bearer(&second_token))
            .await
            .json();
        let names: Vec<_> = listed["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|token| token["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(names, ["phone", "laptop"]);
        assert_eq!(listed["data"][1]["current"], true);
        let phone_id = listed["data"][0]["id"].as_i64().unwrap();

        // Revoking the phone token ends it; the laptop token keeps working.
        server
            .delete(&format!("{BASE}/tokens/{phone_id}"))
            .add_header("authorization", bearer(&second_token))
            .await
            .assert_status_ok();
        server
            .get(&format!("{BASE}/user"))
            .add_header("authorization", bearer(&first_token))
            .await
            .assert_status_unauthorized();
        server
            .delete(&format!("{BASE}/tokens/{phone_id}"))
            .add_header("authorization", bearer(&second_token))
            .await
            .assert_status_not_found();

        server
            .post(&format!("{BASE}/logout"))
            .add_header("authorization", bearer(&second_token))
            .await
            .assert_status_ok();
        server
            .get(&format!("{BASE}/user"))
            .add_header("authorization", bearer(&second_token))
            .await
            .assert_status_unauthorized();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn validation_errors_use_the_laravel_shape() {
    request::<App, _, _>(|server, _ctx| async move {
        let response = server
            .post(&format!("{BASE}/register"))
            .json(&json!({"email": "nope", "password": "short"}))
            .await;
        assert_eq!(response.status_code(), 422);
        let body: Value = response.json();
        assert_eq!(
            body["message"],
            "The name field is required. (and 2 more errors)"
        );
        assert!(body["errors"]["name"].is_array());
        assert!(body["errors"]["email"].is_array());
        assert!(body["errors"]["password"].is_array());

        // Not JSON at all behaves like an empty body.
        let response = server.post(&format!("{BASE}/login")).text("not json").await;
        assert_eq!(response.status_code(), 422);
        assert!(response.json::<Value>()["errors"]["email"].is_array());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn bad_credentials_and_bad_tokens_are_rejected_the_same_way() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();

        for (email, password) in [
            (DEMO_EMAIL, "wrong-password"),
            ("ghost@example.com", "whatever1"),
        ] {
            let response = server
                .post(&format!("{BASE}/login"))
                .json(&json!({"email": email, "password": password}))
                .await;
            assert_eq!(response.status_code(), 422);
            assert_eq!(
                response.json::<Value>()["errors"]["email"][0],
                "These credentials do not match our records."
            );
        }

        server
            .get(&format!("{BASE}/user"))
            .await
            .assert_status_unauthorized();
        for token in ["garbage", "1|short", &format!("999999|{}", "a".repeat(64))] {
            let response = server
                .get(&format!("{BASE}/user"))
                .add_header("authorization", bearer(token))
                .await;
            assert_eq!(response.status_code(), 401, "{token}");
            assert_eq!(response.json::<Value>()["message"], "Unauthenticated.");
        }

        // A valid id with the wrong secret does not authenticate either.
        let login = server
            .post(&format!("{BASE}/login"))
            .json(&json!({"email": DEMO_EMAIL, "password": DEMO_PASSWORD}))
            .await;
        let token = login.json::<Value>()["token"].as_str().unwrap().to_string();
        let id = token.split('|').next().unwrap();
        server
            .get(&format!("{BASE}/user"))
            .add_header("authorization", bearer(&format!("{id}|{}", "b".repeat(64))))
            .await
            .assert_status_unauthorized();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn repeated_failures_are_throttled_and_unknown_api_paths_answer_json() {
    request::<App, _, _>(|server, _ctx| async move {
        for _ in 0..5 {
            server
                .post(&format!("{BASE}/login"))
                .json(&json!({"email": "someone@example.com", "password": "nope-nope"}))
                .await;
        }
        let response = server
            .post(&format!("{BASE}/login"))
            .json(&json!({"email": "someone@example.com", "password": "nope-nope"}))
            .await;
        assert_eq!(response.status_code(), 429);

        let missing = server.get(&format!("{BASE}/nothing-here")).await;
        assert_eq!(missing.status_code(), 404);
        assert_eq!(missing.json::<Value>()["message"], "Not found.");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn logout_all_revokes_every_token() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        let mut tokens = Vec::new();
        for _ in 0..2 {
            let login = server
                .post(&format!("{BASE}/login"))
                .json(&json!({"email": DEMO_EMAIL, "password": DEMO_PASSWORD}))
                .await;
            tokens.push(login.json::<Value>()["token"].as_str().unwrap().to_string());
        }
        server
            .post(&format!("{BASE}/logout-all"))
            .add_header("authorization", bearer(&tokens[0]))
            .await
            .assert_status_ok();
        for token in &tokens {
            server
                .get(&format!("{BASE}/user"))
                .add_header("authorization", bearer(token))
                .await
                .assert_status_unauthorized();
        }
    })
    .await;
}
