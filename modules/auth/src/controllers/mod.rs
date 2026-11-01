mod email_links;

use std::{sync::OnceLock, time::Duration};

use axum::{
    extract::Query,
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    Form,
};
use loco_rs::{environment::Environment, hash, prelude::*};
use module_site_layout::Site;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
    guard::{safe_next, same_origin, OptionalAuth},
    session, users,
};

const MAX_FAILED_LOGINS: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Deserialize)]
struct PageQuery {
    next: Option<String>,
    notice: Option<String>,
}

#[derive(Deserialize)]
struct LoginForm {
    email: String,
    password: String,
    next: Option<String>,
}

#[derive(Deserialize)]
struct RegisterForm {
    name: String,
    email: String,
    password: String,
    password_confirmation: String,
}

/// The Google button shows only when the app has a Google client id.
fn google_login_enabled() -> bool {
    std::env::var("GOOGLE_CLIENT_ID").is_ok_and(|id| !id.trim().is_empty())
}

pub(super) fn secure_cookies(ctx: &AppContext) -> bool {
    ctx.environment == Environment::Production
}

pub(super) fn page(
    v: &TeraView,
    template: &str,
    status: StatusCode,
    data: serde_json::Value,
) -> Result<Response> {
    let body = v.render(template, data)?;
    Ok((status, Html(body)).into_response())
}

pub(super) fn redirect_with_session(location: &str, token: &str, secure: bool) -> Response {
    (
        StatusCode::SEE_OTHER,
        [
            (header::SET_COOKIE, session::cookie_header(token, secure)),
            (header::LOCATION, location.to_string()),
        ],
    )
        .into_response()
}

pub(super) fn user_agent(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
}

/// Hashing a throwaway password for unknown emails keeps the response time the same either way.
fn dummy_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| hash::hash_password("not-a-real-password").unwrap_or_default())
}

fn throttle_key(email: &str) -> String {
    format!("login-failures:{email}")
}

#[debug_handler]
async fn login_form(
    ViewEngine(v): ViewEngine<TeraView>,
    site: Site,
    OptionalAuth(user): OptionalAuth,
    Query(query): Query<PageQuery>,
) -> Result<Response> {
    let next = safe_next(query.next.as_deref());
    if user.is_some() {
        return Ok(axum::response::Redirect::to(&next).into_response());
    }
    let notice = query.notice.filter(|key| {
        matches!(
            key.as_str(),
            "login-needed"
                | "login-google-failed"
                | "login-email-verified"
                | "login-password-reset-done"
                | "login-link-invalid"
        )
    });
    page(
        &v,
        "auth/login.html",
        StatusCode::OK,
        site.data(json!({ "next": next, "email": "", "notice": notice, "google_login": google_login_enabled() })),
    )
}

#[debug_handler]
async fn login(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    site: Site,
    Form(form): Form<LoginForm>,
) -> Result<Response> {
    let next = safe_next(form.next.as_deref());
    let email = form.email.trim().to_lowercase();
    let render = |status: StatusCode, error: &str| {
        page(
            &v,
            "auth/login.html",
            status,
            site.data(json!({ "next": next, "email": email, "error": error, "google_login": google_login_enabled() })),
        )
    };

    if !same_origin(&headers) {
        return render(StatusCode::FORBIDDEN, "request-rejected");
    }

    let key = throttle_key(&email);
    let failures = ctx.cache.get::<u32>(&key).await.ok().flatten().unwrap_or(0);
    if failures >= MAX_FAILED_LOGINS {
        return render(StatusCode::TOO_MANY_REQUESTS, "login-throttled");
    }

    let user = users::Entity::find()
        .filter(users::Column::Email.eq(email.as_str()))
        .one(&ctx.db)
        .await?;
    // An unverified person can sign in (like Laravel); `VerifiedAuth` keeps them out of the pages that need a
    // verified address and sends them to the notice page.
    let accepted = match &user {
        Some(user) => hash::verify_password(&form.password, &user.password),
        None => {
            let _ = hash::verify_password(&form.password, dummy_hash());
            false
        }
    };

    let Some(user) = user.filter(|_| accepted) else {
        let _ = ctx
            .cache
            .insert_with_expiry(&key, &(failures + 1), LOCKOUT)
            .await;
        return render(StatusCode::UNAUTHORIZED, "login-error");
    };

    let _ = ctx.cache.remove(&key).await;
    let token = session::create(&ctx.db, user.id, user_agent(&headers)).await?;
    loco_activity_log::activity(&ctx.db)
        .in_log("auth")
        .caused_by("user", user.id)
        .event("login")
        .log_quietly("Logged in")
        .await;
    Ok(redirect_with_session(&next, &token, secure_cookies(&ctx)))
}

#[debug_handler]
async fn register_form(
    ViewEngine(v): ViewEngine<TeraView>,
    site: Site,
    OptionalAuth(user): OptionalAuth,
) -> Result<Response> {
    if user.is_some() {
        return Ok(axum::response::Redirect::to("/").into_response());
    }
    page(
        &v,
        "auth/register.html",
        StatusCode::OK,
        site.data(json!({ "name": "", "email": "" })),
    )
}

pub fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    email.len() <= 254
        && !local.is_empty()
        && !email.contains(char::is_whitespace)
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !domain.contains('@')
}

#[debug_handler]
async fn register(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    site: Site,
    Form(form): Form<RegisterForm>,
) -> Result<Response> {
    let name = form.name.trim().to_string();
    let email = form.email.trim().to_lowercase();
    let render = |status: StatusCode, error: &str| {
        page(
            &v,
            "auth/register.html",
            status,
            site.data(json!({ "name": name, "email": email, "error": error })),
        )
    };

    if !same_origin(&headers) {
        return render(StatusCode::FORBIDDEN, "request-rejected");
    }
    let name_length = name.chars().count();
    if !(2..=80).contains(&name_length) {
        return render(StatusCode::UNPROCESSABLE_ENTITY, "register-name-invalid");
    }
    if !valid_email(&email) {
        return render(StatusCode::UNPROCESSABLE_ENTITY, "register-email-invalid");
    }
    if form.password.chars().count() < 8 || form.password.len() > 128 {
        return render(StatusCode::UNPROCESSABLE_ENTITY, "register-password-short");
    }
    if form.password != form.password_confirmation {
        return render(
            StatusCode::UNPROCESSABLE_ENTITY,
            "register-password-mismatch",
        );
    }
    if users::Entity::find()
        .filter(users::Column::Email.eq(email.as_str()))
        .one(&ctx.db)
        .await?
        .is_some()
    {
        return render(StatusCode::CONFLICT, "register-email-taken");
    }

    let created = users::ActiveModel {
        pid: Set(Uuid::new_v4()),
        email: Set(email.clone()),
        password: Set(hash::hash_password(&form.password)?),
        api_key: Set(format!("lo-{}", Uuid::new_v4())),
        name: Set(name.clone()),
        // Verified by the link in the welcome mail.
        email_verified_at: Set(None),
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
            return render(StatusCode::CONFLICT, "register-email-taken");
        }
        Err(error) => return Err(error.into()),
    };

    loco_activity_log::activity(&ctx.db)
        .in_log("auth")
        .caused_by("user", user.id)
        .performed_on("user", user.id)
        .event("registered")
        .log_quietly("Registered an account")
        .await;
    email_links::send_verification_link(&v, &ctx, &site, &user).await?;
    let token = session::create(&ctx.db, user.id, user_agent(&headers)).await?;
    Ok(redirect_with_session(
        "/email/verify",
        &token,
        secure_cookies(&ctx),
    ))
}

#[debug_handler]
async fn logout(State(ctx): State<AppContext>, headers: HeaderMap) -> Result<Response> {
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok());
    if let Some(token) = session::token_from_cookie_header(cookie) {
        if let Some((_, user)) = session::find(&ctx.db, token).await? {
            loco_activity_log::activity(&ctx.db)
                .in_log("auth")
                .caused_by("user", user.id)
                .event("logout")
                .log_quietly("Logged out")
                .await;
        }
        session::delete_by_token(&ctx.db, token).await?;
    }
    Ok((
        StatusCode::SEE_OTHER,
        [
            (
                header::SET_COOKIE,
                session::clearing_cookie_header(secure_cookies(&ctx)),
            ),
            (header::LOCATION, "/".to_string()),
        ],
    )
        .into_response())
}

pub fn routes() -> Vec<Routes> {
    vec![
        Routes::new()
            .add("/login", get(login_form).post(login))
            .add("/register", get(register_form).post(register))
            .add("/logout", post(logout)),
        email_links::routes(),
    ]
}

#[cfg(test)]
mod tests {
    use super::valid_email;

    #[test]
    fn email_shape_is_checked() {
        for ok in ["a@b.co", "first.last@mail.example.com"] {
            assert!(valid_email(ok), "{ok}");
        }
        for bad in [
            "", "a", "a@b", "@b.co", "a b@c.co", "a@.co", "a@b.", "a@@b.co",
        ] {
            assert!(!valid_email(bad), "{bad}");
        }
    }
}
