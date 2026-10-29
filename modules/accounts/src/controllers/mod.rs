mod admin_users;

use axum::{
    extract::{Multipart, Query},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use loco_rs::{hash, prelude::*};
use module_auth::{same_origin, session, users, Auth};
use module_media::{read_form, InteractsWithMedia};
use module_site_layout::Site;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::Deserialize;
use serde_json::json;

use crate::logo::{user_owner, LOGO_COLLECTION};

/// Fluent keys a redirect may ask the page to show. Anything else is ignored.
const STATUS_KEYS: [&str; 4] = [
    "profile-saved",
    "password-changed",
    "logo-saved",
    "logo-removed",
];

#[derive(Deserialize)]
struct StatusQuery {
    status: Option<String>,
}

fn status_key(query: &StatusQuery) -> Option<&'static str> {
    STATUS_KEYS
        .into_iter()
        .find(|key| Some(*key) == query.status.as_deref())
}

fn render(
    v: &TeraView,
    template: &str,
    status: StatusCode,
    data: serde_json::Value,
) -> Result<Response> {
    Ok((status, Html(v.render(template, data)?)).into_response())
}

fn rejected() -> Response {
    (StatusCode::FORBIDDEN, "request rejected").into_response()
}

#[debug_handler]
async fn index(Auth(_user): Auth) -> Redirect {
    Redirect::to("/account/profile")
}

#[debug_handler]
async fn profile(
    ViewEngine(v): ViewEngine<TeraView>,
    site: Site,
    Auth(user): Auth,
    Query(query): Query<StatusQuery>,
) -> Result<Response> {
    render(
        &v,
        "accounts/profile.html",
        StatusCode::OK,
        site.data(json!({
            "active": "profile", "status": status_key(&query), "name": user.name,
            "email": user.email, "role": user.access.role.as_ref().map(|role| role.name.clone()),
            "joined": user.created_at.format("%Y-%m-%d").to_string(),
        })),
    )
}

#[derive(Deserialize)]
struct ProfileForm {
    name: String,
}

#[debug_handler]
async fn update_profile(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    site: Site,
    Auth(user): Auth,
    Form(form): Form<ProfileForm>,
) -> Result<Response> {
    if !same_origin(&headers) {
        return Ok(rejected());
    }
    let name = form.name.trim().to_string();
    if !(2..=80).contains(&name.chars().count()) {
        return render(
            &v,
            "accounts/profile.html",
            StatusCode::UNPROCESSABLE_ENTITY,
            site.data(json!({
                "active": "profile", "error": "profile-name-invalid", "name": name, "email": user.email,
                "role": user.access.role.as_ref().map(|role| role.name.clone()),
                "joined": user.created_at.format("%Y-%m-%d").to_string(),
            })),
        );
    }
    let Some(row) = users::Entity::find_by_id(user.id).one(&ctx.db).await? else {
        return Err(Error::NotFound);
    };
    let mut active: users::ActiveModel = row.into();
    active.name = Set(name);
    active.update(&ctx.db).await?;
    Ok(Redirect::to("/account/profile?status=profile-saved").into_response())
}

#[debug_handler]
async fn password_form(
    ViewEngine(v): ViewEngine<TeraView>,
    site: Site,
    Auth(_user): Auth,
    Query(query): Query<StatusQuery>,
) -> Result<Response> {
    render(
        &v,
        "accounts/password.html",
        StatusCode::OK,
        site.data(json!({ "active": "password", "status": status_key(&query) })),
    )
}

#[derive(Deserialize)]
struct PasswordForm {
    current_password: String,
    password: String,
    password_confirmation: String,
}

#[debug_handler]
async fn update_password(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    site: Site,
    Auth(user): Auth,
    Form(form): Form<PasswordForm>,
) -> Result<Response> {
    if !same_origin(&headers) {
        return Ok(rejected());
    }
    let fail = |status: StatusCode, error: &str| {
        render(
            &v,
            "accounts/password.html",
            status,
            site.data(json!({ "active": "password", "error": error })),
        )
    };

    let Some(row) = users::Entity::find_by_id(user.id).one(&ctx.db).await? else {
        return Err(Error::NotFound);
    };
    if !hash::verify_password(&form.current_password, &row.password) {
        return fail(StatusCode::UNPROCESSABLE_ENTITY, "password-wrong");
    }
    if form.password.chars().count() < 8 || form.password.len() > 128 {
        return fail(StatusCode::UNPROCESSABLE_ENTITY, "password-too-short");
    }
    if form.password != form.password_confirmation {
        return fail(StatusCode::UNPROCESSABLE_ENTITY, "password-mismatch");
    }

    let mut active: users::ActiveModel = row.into();
    active.password = Set(hash::hash_password(&form.password)?);
    active.update(&ctx.db).await?;
    // Anyone who knew the old password loses their session; this device stays signed in.
    session::delete_others(&ctx.db, user.id, user.session_id).await?;
    Ok(Redirect::to("/account/password?status=password-changed").into_response())
}

#[debug_handler]
async fn logo_form(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    site: Site,
    Auth(user): Auth,
    Query(query): Query<StatusQuery>,
) -> Result<Response> {
    let logo = user_owner(user.id)
        .get_first_media(&ctx, LOGO_COLLECTION)
        .await?;
    render(
        &v,
        "accounts/logo.html",
        StatusCode::OK,
        site.data(json!({ "active": "logo", "status": status_key(&query), "logo_url": logo.map(|media| media.url()) })),
    )
}

#[debug_handler]
async fn upload_logo(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    site: Site,
    Auth(user): Auth,
    multipart: Multipart,
) -> Result<Response> {
    if !same_origin(&headers) {
        return Ok(rejected());
    }
    let owner = user_owner(user.id);
    let outcome = match read_form(multipart, "logo").await.into_file() {
        Ok(file) => {
            owner
                .add_media(&ctx, file)
                .using_name("Logo")
                .to_media_collection(LOGO_COLLECTION)
                .await
        }
        Err(error) => Err(error),
    };
    match outcome {
        Ok(_) => Ok(Redirect::to("/account/logo?status=logo-saved").into_response()),
        Err(error) if error.is_user_error() => {
            let logo = owner.get_first_media(&ctx, LOGO_COLLECTION).await?;
            render(
                &v,
                "accounts/logo.html",
                StatusCode::UNPROCESSABLE_ENTITY,
                site.data(json!({ "active": "logo", "error": error.message_key(), "logo_url": logo.map(|media| media.url()) })),
            )
        }
        Err(error) => Err(error.into()),
    }
}

#[debug_handler]
async fn remove_logo(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Auth(user): Auth,
) -> Result<Response> {
    if !same_origin(&headers) {
        return Ok(rejected());
    }
    user_owner(user.id)
        .clear_media_collection(&ctx, LOGO_COLLECTION)
        .await?;
    Ok(Redirect::to("/account/logo?status=logo-removed").into_response())
}

pub fn routes() -> Vec<Routes> {
    let mut all = admin_users::routes();
    all.extend(vec![Routes::new()
        .add("/account", get(index))
        .add("/account/profile", get(profile).post(update_profile))
        .add(
            "/account/password",
            get(password_form).post(update_password),
        )
        .add("/account/logo", get(logo_form).post(upload_logo))
        .add("/account/logo/remove", post(remove_logo))]);
    all
}
