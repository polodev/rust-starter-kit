//! Laravel-style token API under `/api/v1/auth`:
//!
//! | Method | Path | Needs token | Reply |
//! |---|---|---|---|
//! | POST | `/register` | no | 201 `{ "user", "token" }` |
//! | POST | `/login` | no | 200 `{ "user", "token" }` |
//! | GET | `/user` | yes | `{ "data": user }` |
//! | POST | `/logout` | yes | revokes this token |
//! | POST | `/logout-all` | yes | revokes every token of the user |
//! | GET | `/tokens` | yes | `{ "data": [token] }` |
//! | DELETE | `/tokens/{id}` | yes | revokes one token |

use std::{sync::OnceLock, time::Duration};

use axum::{
    body::Bytes,
    extract::Path,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
};
use chrono::Utc;
use loco_rs::{hash, prelude::*};
use module_auth::{users, valid_email};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

use crate::{
    api_token,
    guard::{ApiAuth, ApiUser},
    json::{self, Errors, Input},
};

const MAX_FAILED_LOGINS: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(15 * 60);
const DEFAULT_DEVICE: &str = "api";

/// Hashing a throwaway password for unknown emails keeps the response time the same either way.
fn dummy_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| hash::hash_password("not-a-real-password").unwrap_or_default())
}

fn user_json(user: &ApiUser) -> serde_json::Value {
    let mut permissions: Vec<&String> = user.access.permissions.iter().collect();
    permissions.sort();
    json!({
        "id": user.id,
        "name": user.name,
        "email": user.email,
        "role": user.access.role.as_ref().map(|role| role.name.clone()),
        "permissions": permissions,
        "created_at": user.created_at.to_rfc3339(),
    })
}

fn model_json(user: &users::Model) -> serde_json::Value {
    json!({
        "id": user.id,
        "name": user.name,
        "email": user.email,
        "created_at": user.created_at.to_rfc3339(),
    })
}

fn issued(status: StatusCode, user: &users::Model, token: &str) -> Response {
    (
        status,
        axum::Json(json!({ "user": model_json(user), "token": token })),
    )
        .into_response()
}

fn device_name(input: &Input) -> String {
    input
        .text("device_name")
        .unwrap_or_else(|| DEFAULT_DEVICE.to_string())
}

async fn register(State(ctx): State<AppContext>, body: Bytes) -> Result<Response> {
    let input = Input::parse(&body);
    let mut errors = Errors::default();

    let name = input.text("name");
    match &name {
        None => errors.add("name", "The name field is required."),
        Some(name) if !(2..=80).contains(&name.chars().count()) => {
            errors.add(
                "name",
                "The name field must be between 2 and 80 characters.",
            );
        }
        _ => {}
    }

    let email = input.text("email").map(|email| email.to_lowercase());
    match &email {
        None => errors.add("email", "The email field is required."),
        Some(email) if !valid_email(email) => {
            errors.add("email", "The email field must be a valid email address.");
        }
        Some(email) => {
            let taken = users::Entity::find()
                .filter(users::Column::Email.eq(email.as_str()))
                .one(&ctx.db)
                .await?
                .is_some();
            if taken {
                errors.add("email", "The email has already been taken.");
            }
        }
    }

    let password = input.raw("password");
    match &password {
        None => errors.add("password", "The password field is required."),
        Some(password) if password.chars().count() < 8 => {
            errors.add(
                "password",
                "The password field must be at least 8 characters.",
            );
        }
        Some(password) if password.len() > 128 => {
            errors.add(
                "password",
                "The password field must not be greater than 128 characters.",
            );
        }
        Some(password) if input.raw("password_confirmation").as_ref() != Some(password) => {
            errors.add(
                "password",
                "The password field confirmation does not match.",
            );
        }
        _ => {}
    }

    let (Some(name), Some(email), Some(password)) = (name, email, password) else {
        return Ok(errors.into_response());
    };
    if !errors.is_empty() {
        return Ok(errors.into_response());
    }

    let created = users::ActiveModel {
        pid: Set(Uuid::new_v4()),
        email: Set(email.clone()),
        password: Set(hash::hash_password(&password)?),
        api_key: Set(format!("lo-{}", Uuid::new_v4())),
        name: Set(name),
        // No mailer is wired yet, so accounts are marked verified on sign-up (same as the web form).
        email_verified_at: Set(Some(Utc::now().fixed_offset())),
        ..Default::default()
    }
    .insert(&ctx.db)
    .await;

    let user = match created {
        Ok(user) => user,
        // Two sign-ups racing for the same email: the unique index decides.
        Err(_)
            if users::Entity::find()
                .filter(users::Column::Email.eq(email.as_str()))
                .one(&ctx.db)
                .await?
                .is_some() =>
        {
            errors.add("email", "The email has already been taken.");
            return Ok(errors.into_response());
        }
        Err(error) => return Err(error.into()),
    };

    let (token, _) = api_token::create(&ctx.db, user.id, &device_name(&input), None).await?;
    Ok(issued(StatusCode::CREATED, &user, &token))
}

async fn login(State(ctx): State<AppContext>, body: Bytes) -> Result<Response> {
    let input = Input::parse(&body);
    let mut errors = Errors::default();
    let email = input.text("email").map(|email| email.to_lowercase());
    let password = input.raw("password");
    if email.is_none() {
        errors.add("email", "The email field is required.");
    }
    if password.is_none() {
        errors.add("password", "The password field is required.");
    }
    let (Some(email), Some(password)) = (email, password) else {
        return Ok(errors.into_response());
    };

    let key = format!("api-login-failures:{email}");
    let failures = ctx.cache.get::<u32>(&key).await.ok().flatten().unwrap_or(0);
    if failures >= MAX_FAILED_LOGINS {
        return Ok(json::message(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many login attempts. Please try again later.",
        ));
    }

    let user = users::Entity::find()
        .filter(users::Column::Email.eq(email.as_str()))
        .one(&ctx.db)
        .await?;
    let accepted = match &user {
        Some(user) => {
            hash::verify_password(&password, &user.password) && user.email_verified_at.is_some()
        }
        None => {
            let _ = hash::verify_password(&password, dummy_hash());
            false
        }
    };

    let Some(user) = user.filter(|_| accepted) else {
        let _ = ctx
            .cache
            .insert_with_expiry(&key, &(failures + 1), LOCKOUT)
            .await;
        // Laravel's wording: the same error whether the email or the password was wrong.
        errors.add("email", "These credentials do not match our records.");
        return Ok(errors.into_response());
    };

    let _ = ctx.cache.remove(&key).await;
    let (token, _) = api_token::create(&ctx.db, user.id, &device_name(&input), None).await?;
    Ok(issued(StatusCode::OK, &user, &token))
}

async fn user(ApiAuth(user): ApiAuth) -> Response {
    axum::Json(json!({ "data": user_json(&user) })).into_response()
}

async fn logout(State(ctx): State<AppContext>, ApiAuth(user): ApiAuth) -> Result<Response> {
    api_token::revoke(&ctx.db, user.id, user.token_id).await?;
    Ok(json::message(StatusCode::OK, "Logged out."))
}

async fn logout_all(State(ctx): State<AppContext>, ApiAuth(user): ApiAuth) -> Result<Response> {
    api_token::revoke_others(&ctx.db, user.id, 0).await?;
    Ok(json::message(StatusCode::OK, "Logged out of all devices."))
}

async fn tokens(State(ctx): State<AppContext>, ApiAuth(user): ApiAuth) -> Result<Response> {
    let data: Vec<_> = api_token::for_user(&ctx.db, user.id)
        .await?
        .into_iter()
        .map(|token| {
            json!({
                "id": token.id,
                "name": token.name,
                "current": token.id == user.token_id,
                "last_used_at": token.last_used_at.map(|at| at.to_rfc3339()),
                "expires_at": token.expires_at.map(|at| at.to_rfc3339()),
                "created_at": token.created_at.to_rfc3339(),
            })
        })
        .collect();
    Ok(axum::Json(json!({ "data": data })).into_response())
}

async fn revoke_token(
    State(ctx): State<AppContext>,
    ApiAuth(user): ApiAuth,
    Path(id): Path<i64>,
) -> Result<Response> {
    if api_token::revoke(&ctx.db, user.id, id).await? {
        Ok(json::message(StatusCode::OK, "Token revoked."))
    } else {
        Ok(json::message(StatusCode::NOT_FOUND, "Not found."))
    }
}

/// Unknown `/api/...` paths answer JSON, never an HTML page.
async fn not_found(uri: Uri) -> Response {
    tracing::debug!(%uri, "unknown api route");
    json::message(StatusCode::NOT_FOUND, "Not found.")
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new()
        .prefix("/api/v1/auth")
        .add("/register", post(register))
        .add("/login", post(login))
        .add("/user", get(user))
        .add("/logout", post(logout))
        .add("/logout-all", post(logout_all))
        .add("/tokens", get(tokens))
        .add("/tokens/{id}", delete(revoke_token))
        .add("/{*rest}", any(not_found))]
}
