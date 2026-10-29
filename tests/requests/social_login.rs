use std::net::SocketAddr;

use axum::{
    routing::{get, post},
    Json, Router,
};
use loco_rs::testing::prelude::*;
use module_auth::users;
use rust_starter::app::App;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde_json::{json, Value};
use serial_test::serial;

const CLIENT_ID: &str = "test-client.apps.googleusercontent.com";

/// A fake Google on a random local port; the app is pointed at it through environment variables.
async fn fake_google() {
    let profile = || async {
        Json(
            json!({"sub": "g-123", "email": "Person@Gmail.com", "email_verified": true, "name": "Google Person"}),
        )
    };
    let app = Router::new()
        .route("/token", post(|| async { Json(json!({"access_token": "at"})) }))
        .route("/userinfo", get(profile))
        .route(
            "/tokeninfo",
            get(|axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>| async move {
                match q.get("id_token").map(String::as_str) {
                    Some("good") => (axum::http::StatusCode::OK, Json(json!({"sub": "g-123", "aud": CLIENT_ID, "email": "person@gmail.com", "email_verified": "true", "name": "Google Person"}))),
                    Some("other-app") => (axum::http::StatusCode::OK, Json(json!({"sub": "g-9", "aud": "someone-else", "email": "x@gmail.com", "email_verified": "true"}))),
                    Some("unverified") => (axum::http::StatusCode::OK, Json(json!({"sub": "g-8", "aud": CLIENT_ID, "email": "y@gmail.com", "email_verified": "false"}))),
                    _ => (axum::http::StatusCode::BAD_REQUEST, Json(json!({"error": "invalid_token"}))),
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    std::env::set_var("GOOGLE_CLIENT_ID", CLIENT_ID);
    std::env::set_var("GOOGLE_CLIENT_SECRET", "secret");
    std::env::set_var("GOOGLE_TOKEN_URL", format!("http://{addr}/token"));
    std::env::set_var("GOOGLE_USERINFO_URL", format!("http://{addr}/userinfo"));
    std::env::set_var("GOOGLE_TOKENINFO_URL", format!("http://{addr}/tokeninfo"));
}

fn cookie_pairs(response: &axum_test::TestResponse) -> Vec<String> {
    response
        .iter_headers_by_name("set-cookie")
        .map(|value| {
            value
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_string()
        })
        .collect()
}

#[tokio::test]
#[serial]
async fn browser_flow_checks_state_creates_the_user_and_signs_in() {
    request::<App, _, _>(|server, ctx| async move {
        fake_google().await;

        let start = server.get("/auth/google/redirect").await;
        assert_eq!(start.status_code(), 303);
        let location = start.header("location").to_str().unwrap().to_string();
        assert!(location.contains("client_id=test-client") && location.contains("scope=openid"));
        let state_cookie = cookie_pairs(&start).remove(0);
        let state = state_cookie.split_once('=').unwrap().1.to_string();
        assert!(location.contains(&format!("state={state}")));

        // A callback with the wrong state is refused.
        let forged = server
            .get("/auth/google/callback?code=abc&state=forged")
            .add_header("cookie", state_cookie.clone())
            .await;
        assert!(forged
            .header("location")
            .to_str()
            .unwrap()
            .contains("login-google-failed"));
        assert_eq!(users::Entity::find().count(&ctx.db).await.unwrap(), 0);

        let done = server
            .get(&format!("/auth/google/callback?code=abc&state={state}"))
            .add_header("cookie", state_cookie)
            .await;
        assert_eq!(done.status_code(), 303);
        assert_eq!(done.header("location").to_str().unwrap(), "/");
        let session = cookie_pairs(&done)
            .into_iter()
            .find(|c| c.starts_with("session="))
            .unwrap();

        let created = users::Entity::find()
            .filter(users::Column::Email.eq("person@gmail.com"))
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(created.name, "Google Person");
        assert!(created.email_verified_at.is_some());

        let home = server.get("/").add_header("cookie", session).await.text();
        assert!(home.contains(r#"data-testid="user-menu""#));

        // Signing in again finds the same account.
        let again_state = "a".repeat(64);
        server
            .get(&format!(
                "/auth/google/callback?code=abc&state={again_state}"
            ))
            .add_header("cookie", format!("google_oauth_state={again_state}"))
            .await;
        assert_eq!(users::Entity::find().count(&ctx.db).await.unwrap(), 1);

        // The login page offers the button now.
        let login = server.get("/login").await.text();
        assert!(login.contains(r#"data-testid="google-login""#));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn api_flow_exchanges_a_google_id_token_for_a_bearer_token() {
    request::<App, _, _>(|server, ctx| async move {
        fake_google().await;

        let ok = server
            .post("/api/v1/auth/google")
            .json(&json!({"id_token": "good", "device_name": "pixel"}))
            .await;
        ok.assert_status_ok();
        let body: Value = ok.json();
        assert_eq!(body["user"]["email"], "person@gmail.com");
        let token = body["token"].as_str().unwrap().to_string();

        let me: Value = server
            .get("/api/v1/auth/user")
            .add_header("authorization", format!("Bearer {token}"))
            .await
            .json();
        assert_eq!(me["data"]["email"], "person@gmail.com");

        assert_eq!(body["data"]["token"], body["token"]);
        assert_eq!(body["message"], "Login successful.");

        // A second client id (iOS app) is accepted when listed, and `access_token` is an alias.
        std::env::set_var("GOOGLE_CLIENT_IDS", "ios-client, someone-else");
        let alias = server
            .post("/api/v1/auth/google")
            .json(&json!({"access_token": "other-app", "token_type": "google"}))
            .await;
        alias.assert_status_ok();
        std::env::remove_var("GOOGLE_CLIENT_IDS");

        for bad in ["nope", "other-app", "unverified"] {
            let response = server
                .post("/api/v1/auth/google")
                .json(&json!({"id_token": bad}))
                .await;
            assert_eq!(response.status_code(), 401, "{bad}");
        }
        let missing = server.post("/api/v1/auth/google").json(&json!({})).await;
        assert_eq!(missing.status_code(), 422);
        assert_eq!(users::Entity::find().count(&ctx.db).await.unwrap(), 2);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn an_existing_account_with_the_same_email_is_linked_not_duplicated() {
    request::<App, _, _>(|server, ctx| async move {
        fake_google().await;
        let existing = rust_starter::models::users::Model::create_with_password(
            &ctx.db,
            &rust_starter::models::users::RegisterParams {
                email: "person@gmail.com".into(),
                password: "password123".into(),
                name: "Old Name".into(),
            },
        )
        .await
        .unwrap();

        let response = server
            .post("/api/v1/auth/google")
            .json(&json!({"id_token": "good"}))
            .await;
        response.assert_status_ok();
        assert_eq!(response.json::<Value>()["user"]["id"], existing.id);
        assert_eq!(users::Entity::find().count(&ctx.db).await.unwrap(), 1);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn match_only_mode_refuses_unknown_people_but_lets_existing_ones_in() {
    request::<App, _, _>(|server, ctx| async move {
        fake_google().await;
        std::env::set_var("GOOGLE_AUTO_CREATE", "false");

        let unknown = server
            .post("/api/v1/auth/google")
            .json(&json!({"id_token": "good"}))
            .await;
        assert_eq!(unknown.status_code(), 404);
        assert_eq!(users::Entity::find().count(&ctx.db).await.unwrap(), 0);

        rust_starter::models::users::Model::create_with_password(
            &ctx.db,
            &rust_starter::models::users::RegisterParams {
                email: "person@gmail.com".into(),
                password: "password123".into(),
                name: "Known".into(),
            },
        )
        .await
        .unwrap();
        server
            .post("/api/v1/auth/google")
            .json(&json!({"id_token": "good"}))
            .await
            .assert_status_ok();
        std::env::remove_var("GOOGLE_AUTO_CREATE");
    })
    .await;
}
