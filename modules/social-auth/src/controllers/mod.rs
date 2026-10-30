//! Sign in with Google (the Laravel Socialite idea), for the website and for the API.
//!
//! * `GET  /auth/google/redirect`   starts the browser flow
//! * `GET  /auth/google/callback`   Google sends the person back here; a session cookie is set
//! * `POST /api/v1/auth/google`     `{ "id_token", "device_name" }` returns `{ "user", "token", "data", "message" }`

use axum::{
    body::Bytes,
    extract::Query,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use loco_rs::{environment::Environment, prelude::*};
use module_auth::{session, users};
use serde::Deserialize;
use serde_json::json;

use crate::{
    google::{self, GoogleError},
    service::find_or_create_user,
};

const STATE_COOKIE: &str = "google_oauth_state";
const STATE_MAX_AGE: u32 = 600;

fn secure(ctx: &AppContext) -> &'static str {
    if ctx.environment == Environment::Production {
        "; Secure"
    } else {
        ""
    }
}

fn state_from(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|cookie| cookie.trim().strip_prefix(&format!("{STATE_COOKIE}=")))
        .map(str::to_string)
}

fn state_cookie(value: &str, max_age: u32, secure: &str) -> String {
    format!(
        "{STATE_COOKIE}={value}; Path=/auth/google; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}"
    )
}

fn failed(ctx: &AppContext) -> Response {
    (
        StatusCode::SEE_OTHER,
        [
            (
                header::LOCATION,
                "/login?notice=login-google-failed".to_string(),
            ),
            (header::SET_COOKIE, state_cookie("", 0, secure(ctx))),
        ],
    )
        .into_response()
}

async fn redirect(State(ctx): State<AppContext>) -> Response {
    let state = session::new_token();
    match google::authorization_url(&ctx.config.server.full_url(), &state) {
        Ok(url) => (
            StatusCode::SEE_OTHER,
            [
                (header::LOCATION, url),
                (
                    header::SET_COOKIE,
                    state_cookie(&state, STATE_MAX_AGE, secure(&ctx)),
                ),
            ],
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Google sign-in is not set up").into_response(),
    }
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn callback(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Result<Response> {
    let expected = state_from(&headers).filter(|state| state.len() == 64);
    // The `state` we set before leaving must come back unchanged, or the visit was not ours.
    let (Some(code), Some(state), Some(expected), None) =
        (query.code, query.state, expected, query.error)
    else {
        return Ok(failed(&ctx));
    };
    if state != expected {
        return Ok(failed(&ctx));
    }
    let profile = match google::profile_for_code(&ctx.config.server.full_url(), &code).await {
        Ok(profile) => profile,
        Err(error) => {
            tracing::warn!(?error, "google sign-in failed");
            return Ok(failed(&ctx));
        }
    };
    let Some(user) = find_or_create_user(&ctx.db, &profile, google::auto_create()).await? else {
        return Ok(failed(&ctx));
    };
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok());
    let token = session::create(&ctx.db, user.id, user_agent).await?;
    let secure_flag = ctx.environment == Environment::Production;
    let mut response = Redirect::to("/").into_response();
    for cookie in [
        session::cookie_header(&token, secure_flag),
        state_cookie("", 0, secure(&ctx)),
    ] {
        if let Ok(value) = cookie.parse() {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    *response.status_mut() = StatusCode::SEE_OTHER;
    Ok(response)
}

fn user_json(user: &users::Model) -> serde_json::Value {
    json!({
        "id": user.id, "name": user.name, "email": user.email,
        "created_at": user.created_at.to_rfc3339(),
    })
}

async fn api_google(State(ctx): State<AppContext>, body: Bytes) -> Result<Response> {
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    let text = |field: &str| {
        value
            .get(field)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    // `access_token` is accepted as an alias for the same Google ID token, which some mobile clients send under that name.
    let Some(id_token) = text("id_token").or_else(|| text("access_token")) else {
        let mut errors = module_auth_api::ValidationErrors::default();
        errors.add("id_token", "The id token field is required.");
        return Ok(errors.into_response());
    };
    let profile = match google::profile_for_id_token(&id_token).await {
        Ok(profile) => profile,
        Err(GoogleError::NotConfigured) => {
            return Ok(module_auth_api::json_message(
                StatusCode::SERVICE_UNAVAILABLE,
                "Google sign-in is not set up.",
            ));
        }
        Err(GoogleError::Unreachable) => {
            return Ok(module_auth_api::json_message(
                StatusCode::BAD_GATEWAY,
                "Google could not be reached.",
            ));
        }
        Err(GoogleError::Rejected(_)) => return Ok(module_auth_api::unauthenticated()),
    };
    let Some(user) = find_or_create_user(&ctx.db, &profile, google::auto_create()).await? else {
        return Ok(module_auth_api::json_message(
            StatusCode::NOT_FOUND,
            "No account found with this email.",
        ));
    };
    let device = text("device_name").unwrap_or_else(|| "google".to_string());
    let (token, _) = module_auth_api::create_api_token(&ctx.db, user.id, &device, None).await?;
    Ok((
        StatusCode::OK,
        // `data` and `message` repeat the same values for existing mobile clients that read them.
        axum::Json(json!({
            "user": user_json(&user), "token": token,
            "data": { "token": token, "user": user_json(&user) },
            "message": "Login successful.",
        })),
    )
        .into_response())
}

pub fn routes() -> Vec<Routes> {
    vec![
        Routes::new()
            .add("/auth/google/redirect", get(redirect))
            .add("/auth/google/callback", get(callback)),
        Routes::new().add("/api/v1/auth/google", post(api_google)),
    ]
}
