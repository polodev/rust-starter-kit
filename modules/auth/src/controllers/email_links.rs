//! Everything that is sent to the person's inbox: the link that verifies the address, the link that resets a
//! forgotten password, and the link that signs in without a password. Links are one-time and expire
//! (`security_tokens`); the requests never reveal whether an address has an account.

use std::time::Duration;

use axum::{
    extract::Path,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use chrono::Utc;
use loco_rs::{hash, prelude::*};
use module_mail::Mail;
use module_site_layout::Site;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde::Deserialize;
use serde_json::json;

use super::{page, redirect_with_session, secure_cookies, user_agent};
use crate::{
    guard::{same_origin, Auth},
    security_tokens::{self, EMAIL_VERIFICATION, MAGIC_LOGIN, PASSWORD_RESET},
    session, users,
};

/// How many link requests one address may make in the window below.
const MAX_LINK_REQUESTS: u32 = 3;
const LINK_REQUEST_WINDOW: Duration = Duration::from_secs(60 * 60);

#[derive(Deserialize)]
struct EmailForm {
    email: String,
}

#[derive(Deserialize)]
struct ResetForm {
    password: String,
    password_confirmation: String,
}

/// What a link in a mail does and how its mail reads.
struct LinkKind {
    purpose: &'static str,
    url_prefix: &'static str,
    subject_key: &'static str,
    intro_key: &'static str,
    button_key: &'static str,
}

const VERIFY_EMAIL: LinkKind = LinkKind {
    purpose: EMAIL_VERIFICATION,
    url_prefix: "/email/verify/",
    subject_key: "mail-verify-email-subject",
    intro_key: "mail-verify-email-intro",
    button_key: "mail-verify-email-button",
};
const RESET_PASSWORD: LinkKind = LinkKind {
    purpose: PASSWORD_RESET,
    url_prefix: "/reset-password/",
    subject_key: "mail-reset-password-subject",
    intro_key: "mail-reset-password-intro",
    button_key: "mail-reset-password-button",
};
const SIGN_IN_LINK: LinkKind = LinkKind {
    purpose: MAGIC_LOGIN,
    url_prefix: "/login/email-link/",
    subject_key: "mail-sign-in-link-subject",
    intro_key: "mail-sign-in-link-intro",
    button_key: "mail-sign-in-link-button",
};

/// Makes a link and mails it. A mail problem is logged, not shown: the person can ask again, and the answer
/// must look the same whether or not it worked.
async fn send_link(
    v: &TeraView,
    ctx: &AppContext,
    site: &Site,
    user: &users::Model,
    kind: &LinkKind,
) -> Result<()> {
    let secret = security_tokens::issue(&ctx.db, user.id, kind.purpose).await?;
    let data = json!({
        "tag": site.tag(),
        "name": user.name,
        "url": format!("{}{}{}", ctx.config.server.full_url(), kind.url_prefix, secret),
        "subject_key": kind.subject_key,
        "intro_key": kind.intro_key,
        "button_key": kind.button_key,
    });
    let mail = Mail::to(&user.email)
        .subject(v.render("auth/mail_subject.txt", &data)?.trim())
        .html(&v.render("auth/mail_link.html", &data)?)
        .text(&v.render("auth/mail_link.txt", &data)?);
    if let Err(error) = module_mail::dispatch(ctx, mail).await {
        tracing::warn!(user_id = user.id, purpose = kind.purpose, %error, "link email not sent");
    }
    Ok(())
}

/// Sends the verification mail; used by sign-up and by the resend form.
pub(super) async fn send_verification_link(
    v: &TeraView,
    ctx: &AppContext,
    site: &Site,
    user: &users::Model,
) -> Result<()> {
    send_link(v, ctx, site, user, &VERIFY_EMAIL).await
}

/// True while this address is still allowed to ask for another link.
async fn may_request_link(ctx: &AppContext, purpose: &str, email: &str) -> bool {
    let key = format!("link-requests:{purpose}:{email}");
    let made = ctx.cache.get::<u32>(&key).await.ok().flatten().unwrap_or(0);
    if made >= MAX_LINK_REQUESTS {
        return false;
    }
    let _ = ctx
        .cache
        .insert_with_expiry(&key, &(made + 1), LINK_REQUEST_WINDOW)
        .await;
    true
}

async fn user_by_email(ctx: &AppContext, email: &str) -> Result<Option<users::Model>> {
    Ok(users::Entity::find()
        .filter(users::Column::Email.eq(email))
        .one(&ctx.db)
        .await?)
}

/// The shared page with one email field: forgot password, resend verification, sign-in link.
fn request_page(
    v: &TeraView,
    site: &Site,
    status: StatusCode,
    which: &str,
    sent: bool,
    email: &str,
) -> Result<Response> {
    let (action, title, subtitle, submit) = match which {
        "reset" => (
            "/forgot-password",
            "forgot-title",
            "forgot-subtitle",
            "forgot-submit",
        ),
        "verify" => (
            "/email/verification-notification",
            "resend-title",
            "resend-subtitle",
            "resend-submit",
        ),
        _ => (
            "/login/email-link",
            "magic-title",
            "magic-subtitle",
            "magic-submit",
        ),
    };
    page(
        v,
        "auth/email_request.html",
        status,
        site.data(json!({
            "action": action, "title_key": title, "subtitle_key": subtitle, "submit_key": submit,
            "sent": sent, "email": email, "which": which,
        })),
    )
}

/// The same handlers for the three "enter your email, get a link" forms.
async fn request_link(
    v: &TeraView,
    ctx: &AppContext,
    headers: &HeaderMap,
    site: &Site,
    form: EmailForm,
    which: &str,
    kind: &LinkKind,
) -> Result<Response> {
    let email = form.email.trim().to_lowercase();
    if !same_origin(headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    if may_request_link(ctx, kind.purpose, &email).await {
        if let Some(user) = user_by_email(ctx, &email).await? {
            // A verification link is only for an address that is not verified yet.
            let wanted = kind.purpose != EMAIL_VERIFICATION || user.email_verified_at.is_none();
            if wanted {
                send_link(v, ctx, site, &user, kind).await?;
            }
        }
    }
    request_page(v, site, StatusCode::OK, which, true, &email)
}

#[debug_handler]
async fn forgot_password_form(ViewEngine(v): ViewEngine<TeraView>, site: Site) -> Result<Response> {
    request_page(&v, &site, StatusCode::OK, "reset", false, "")
}

#[debug_handler]
async fn forgot_password(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    site: Site,
    Form(form): Form<EmailForm>,
) -> Result<Response> {
    request_link(&v, &ctx, &headers, &site, form, "reset", &RESET_PASSWORD).await
}

#[debug_handler]
async fn resend_verification_form(
    ViewEngine(v): ViewEngine<TeraView>,
    site: Site,
) -> Result<Response> {
    request_page(&v, &site, StatusCode::OK, "verify", false, "")
}

#[debug_handler]
async fn resend_verification(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    site: Site,
    Form(form): Form<EmailForm>,
) -> Result<Response> {
    request_link(&v, &ctx, &headers, &site, form, "verify", &VERIFY_EMAIL).await
}

#[debug_handler]
async fn sign_in_link_form(ViewEngine(v): ViewEngine<TeraView>, site: Site) -> Result<Response> {
    request_page(&v, &site, StatusCode::OK, "magic", false, "")
}

#[debug_handler]
async fn sign_in_link(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    site: Site,
    Form(form): Form<EmailForm>,
) -> Result<Response> {
    request_link(&v, &ctx, &headers, &site, form, "magic", &SIGN_IN_LINK).await
}

/// Marks the address as verified (the person proved they read this mailbox).
async fn mark_verified(ctx: &AppContext, user_id: i64) -> Result<()> {
    if let Some(user) = users::Entity::find_by_id(user_id).one(&ctx.db).await? {
        if user.email_verified_at.is_none() {
            let mut active = user.into_active_model();
            active.email_verified_at = Set(Some(Utc::now().fixed_offset()));
            active.update(&ctx.db).await?;
        }
    }
    Ok(())
}

fn login_with_notice(notice: &str) -> Response {
    Redirect::to(&format!("/login?notice={notice}")).into_response()
}

/// "Verify your email address": where `VerifiedAuth` sends a signed-in person who is not verified yet (Laravel's
/// `verification.notice`).
#[debug_handler]
async fn verification_notice(
    ViewEngine(v): ViewEngine<TeraView>,
    site: Site,
    Auth(user): Auth,
) -> Result<Response> {
    if user.email_verified {
        return Ok(Redirect::to("/").into_response());
    }
    page(
        &v,
        "auth/verify_notice.html",
        StatusCode::OK,
        site.data(json!({ "email": user.email })),
    )
}

#[debug_handler]
async fn verify_email(
    State(ctx): State<AppContext>,
    Path(secret): Path<String>,
) -> Result<Response> {
    match security_tokens::consume(&ctx.db, &secret, EMAIL_VERIFICATION).await? {
        Some(user_id) => {
            mark_verified(&ctx, user_id).await?;
            loco_activity_log::activity(&ctx.db)
                .in_log("auth")
                .caused_by("user", user_id)
                .performed_on("user", user_id)
                .event("email_verified")
                .log_quietly("Verified the email address")
                .await;
            Ok(login_with_notice("login-email-verified"))
        }
        None => Ok(login_with_notice("login-link-invalid")),
    }
}

#[debug_handler]
async fn reset_password_form(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    site: Site,
    Path(secret): Path<String>,
) -> Result<Response> {
    if security_tokens::peek(&ctx.db, &secret, PASSWORD_RESET)
        .await?
        .is_none()
    {
        return Ok(login_with_notice("login-link-invalid"));
    }
    page(
        &v,
        "auth/reset_password.html",
        StatusCode::OK,
        site.data(json!({ "secret": secret })),
    )
}

#[debug_handler]
async fn reset_password(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    site: Site,
    Path(secret): Path<String>,
    Form(form): Form<ResetForm>,
) -> Result<Response> {
    let render = |status: StatusCode, error: &str| {
        page(
            &v,
            "auth/reset_password.html",
            status,
            site.data(json!({ "secret": secret, "error": error })),
        )
    };
    if !same_origin(&headers) {
        return render(StatusCode::FORBIDDEN, "request-rejected");
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
    let Some(user_id) = security_tokens::consume(&ctx.db, &secret, PASSWORD_RESET).await? else {
        return Ok(login_with_notice("login-link-invalid"));
    };
    let Some(user) = users::Entity::find_by_id(user_id).one(&ctx.db).await? else {
        return Ok(login_with_notice("login-link-invalid"));
    };
    let mut active = user.into_active_model();
    active.password = Set(hash::hash_password(&form.password)?);
    // The link came to the mailbox, so it also proves the address.
    if active.email_verified_at.as_ref().is_none() {
        active.email_verified_at = Set(Some(Utc::now().fixed_offset()));
    }
    active.update(&ctx.db).await?;
    // Whoever knew the old password is signed out everywhere.
    session::delete_all_for_user(&ctx.db, user_id).await?;
    loco_activity_log::activity(&ctx.db)
        .in_log("auth")
        .caused_by("user", user_id)
        .performed_on("user", user_id)
        .event("password_reset")
        .log_quietly("Reset the password with an email link")
        .await;
    Ok(login_with_notice("login-password-reset-done"))
}

/// The link opens a page with a button instead of signing in at once: mail scanners that open links ahead of
/// the person would otherwise use the one-time link up.
#[debug_handler]
async fn sign_in_link_confirm(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    site: Site,
    Path(secret): Path<String>,
) -> Result<Response> {
    if security_tokens::peek(&ctx.db, &secret, MAGIC_LOGIN)
        .await?
        .is_none()
    {
        return Ok(login_with_notice("login-link-invalid"));
    }
    page(
        &v,
        "auth/sign_in_link_confirm.html",
        StatusCode::OK,
        site.data(json!({ "secret": secret })),
    )
}

#[debug_handler]
async fn sign_in_link_use(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(secret): Path<String>,
) -> Result<Response> {
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    let Some(user_id) = security_tokens::consume(&ctx.db, &secret, MAGIC_LOGIN).await? else {
        return Ok(login_with_notice("login-link-invalid"));
    };
    mark_verified(&ctx, user_id).await?;
    let token = session::create(&ctx.db, user_id, user_agent(&headers)).await?;
    loco_activity_log::activity(&ctx.db)
        .in_log("auth")
        .caused_by("user", user_id)
        .event("login_by_email_link")
        .log_quietly("Logged in with an email link")
        .await;
    Ok(redirect_with_session("/", &token, secure_cookies(&ctx)))
}

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/forgot-password",
            get(forgot_password_form).post(forgot_password),
        )
        .add(
            "/reset-password/{secret}",
            get(reset_password_form).post(reset_password),
        )
        .add(
            "/email/verification-notification",
            get(resend_verification_form).post(resend_verification),
        )
        .add("/email/verify", get(verification_notice))
        .add("/email/verify/{secret}", get(verify_email))
        .add(
            "/login/email-link",
            get(sign_in_link_form).post(sign_in_link),
        )
        .add(
            "/login/email-link/{secret}",
            get(sign_in_link_confirm).post(sign_in_link_use),
        )
}
