//! Admin screens for one user account: the read-only page, the edit page (name, email, profile
//! image, generated password). The list of users lives in the admin layout module.

use axum::{
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use loco_rs::prelude::*;
use module_admin_layout::AdminPage;
use module_auth::{same_origin, session, users, valid_email};
use module_media::{read_form, InteractsWithMedia, MediaError};
use module_role_permission::access_for;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use serde_json::json;

use crate::logo::{user_owner, LOGO_COLLECTION};

const PASSWORD_ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const GENERATED_PASSWORD_LENGTH: usize = 16;

/// A random password from characters that are hard to confuse (no 0/O, 1/l/I).
fn generate_password() -> String {
    let bytes = [uuid::Uuid::new_v4(), uuid::Uuid::new_v4()].map(|id| *id.as_bytes());
    bytes
        .iter()
        .flatten()
        .take(GENERATED_PASSWORD_LENGTH)
        .map(|byte| PASSWORD_ALPHABET[usize::from(*byte) % PASSWORD_ALPHABET.len()] as char)
        .collect()
}

#[derive(Deserialize)]
struct ProfileForm {
    name: String,
    email: String,
}

fn user_status(saved: Option<&str>, error: Option<&str>) -> Option<serde_json::Value> {
    let (kind, text) = match (saved, error) {
        (Some("profile"), _) => ("ok", "Details saved."),
        (_, Some("name")) => ("error", "The name must be 2 to 80 characters."),
        (_, Some("email")) => ("error", "Enter a valid email address."),
        (_, Some("taken")) => ("error", "Another account already uses that email."),
        (Some("image"), _) => ("ok", "Profile image saved."),
        (Some("image-removed"), _) => ("ok", "Profile image removed."),
        (_, Some("image-empty")) => ("error", "Choose an image first."),
        (_, Some("image-type")) => ("error", "Upload a PNG, JPG or WebP image."),
        (_, Some("image-size")) => ("error", "The image is larger than 1 MB."),
        _ => return None,
    };
    Some(json!({ "kind": kind, "text": text }))
}

async fn user_show_data(
    ctx: &AppContext,
    page: &AdminPage,
    user: &users::Model,
    extra: serde_json::Value,
) -> Result<serde_json::Value> {
    let access = access_for(&ctx.db, user.id).await?;
    let image = user_owner(user.id)
        .get_first_media(ctx, LOGO_COLLECTION)
        .await?
        .map(|media| json!({ "url": media.url(), "name": media.file_name, "kb": media.size.div_ceil(1024) }));
    let mut data = json!({
        "title": user.name,
        "image": image,
        "item": {
            "id": user.id, "name": user.name, "email": user.email,
            "verified": user.email_verified_at.is_some(),
            "joined": user.created_at.format("%Y-%m-%d %H:%M").to_string(),
            "role": access.role.as_ref().map(|role| role.name.clone()),
            "is_you": user.id == page.user.id,
        },
        "can_manage": page.can("users.manage"),
    });
    if let (Some(base), serde_json::Value::Object(extra)) = (data.as_object_mut(), extra) {
        base.extend(extra);
    }
    Ok(data)
}

#[derive(Deserialize)]
struct ErrorQuery {
    saved: Option<String>,
    error: Option<String>,
}

#[debug_handler]
async fn user_show(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Path(user_id): Path<i64>,
    Query(query): Query<ErrorQuery>,
) -> Result<Response> {
    if !page.can("users.view") {
        return page.forbidden(&v);
    }
    let user = users::Entity::find_by_id(user_id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    let status = user_status(query.saved.as_deref(), query.error.as_deref());
    let data = user_show_data(&ctx, &page, &user, json!({ "status": status })).await?;
    page.render(&v, "admin/user_show.html", data)
}

#[debug_handler]
async fn user_edit(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    page: AdminPage,
    Path(user_id): Path<i64>,
    Query(query): Query<ErrorQuery>,
) -> Result<Response> {
    if !page.can("users.manage") {
        return page.forbidden(&v);
    }
    let user = users::Entity::find_by_id(user_id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    let status = user_status(query.saved.as_deref(), query.error.as_deref());
    let data = user_show_data(&ctx, &page, &user, json!({ "status": status })).await?;
    page.render(&v, "admin/user_edit.html", data)
}

#[debug_handler]
async fn user_update(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(user_id): Path<i64>,
    Form(form): Form<ProfileForm>,
) -> Result<Response> {
    if !page.can("users.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    let user = users::Entity::find_by_id(user_id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    let name = form.name.trim().to_string();
    let email = form.email.trim().to_lowercase();
    let back = |error: &str| {
        Redirect::to(&format!("/admin/users/{user_id}/edit?error={error}")).into_response()
    };

    if !(2..=80).contains(&name.chars().count()) {
        return Ok(back("name"));
    }
    if !valid_email(&email) {
        return Ok(back("email"));
    }
    if email != user.email
        && users::Entity::find()
            .filter(users::Column::Email.eq(email.as_str()))
            .one(&ctx.db)
            .await?
            .is_some()
    {
        return Ok(back("taken"));
    }

    let old = json!({ "name": user.name, "email": user.email });
    let new = json!({ "name": name, "email": email });
    let mut active: users::ActiveModel = user.into();
    active.name = Set(name);
    active.email = Set(email.clone());
    active.updated_at = Set(chrono::Utc::now().fixed_offset());
    // Two admins racing for one email: the unique index decides.
    if active.update(&ctx.db).await.is_err() {
        return Ok(back("taken"));
    }
    loco_activity_log::activity(&ctx.db)
        .in_log("users")
        .caused_by("user", page.user.id)
        .performed_on("user", user_id)
        .event("updated")
        .with_changes(loco_activity_log::changes(&old, &new, &[]))
        .log_quietly("Updated the user's details")
        .await;
    Ok(Redirect::to(&format!("/admin/users/{user_id}?saved=profile")).into_response())
}

fn image_back(user_id: i64, key: &str, value: &str) -> Response {
    Redirect::to(&format!("/admin/users/{user_id}/edit?{key}={value}")).into_response()
}

/// The profile image is the user's own "logo" collection (a single file: a new upload replaces
/// the old one), so it also shows as their avatar in the site header.
#[debug_handler]
async fn user_image_upload(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(user_id): Path<i64>,
    multipart: Multipart,
) -> Result<Response> {
    if !page.can("users.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    let user = users::Entity::find_by_id(user_id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    let file = match read_form(multipart, "image").await.into_file() {
        Ok(file) => file,
        Err(MediaError::TooLarge { .. }) => return Ok(image_back(user_id, "error", "image-size")),
        Err(_) => return Ok(image_back(user_id, "error", "image-empty")),
    };
    let result = user_owner(user.id)
        .add_media(&ctx, file)
        .using_name("Logo")
        .to_media_collection(LOGO_COLLECTION)
        .await;
    match result {
        Ok(_) => {
            loco_activity_log::activity(&ctx.db)
                .in_log("users")
                .caused_by("user", page.user.id)
                .performed_on("user", user_id)
                .event("image_changed")
                .log_quietly("Changed the profile image")
                .await;
            Ok(image_back(user_id, "saved", "image"))
        }
        Err(MediaError::Empty) => Ok(image_back(user_id, "error", "image-empty")),
        Err(MediaError::UnsupportedType) => Ok(image_back(user_id, "error", "image-type")),
        Err(MediaError::TooLarge { .. }) => Ok(image_back(user_id, "error", "image-size")),
        Err(error) => Err(error.into()),
    }
}

#[debug_handler]
async fn user_image_remove(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(user_id): Path<i64>,
) -> Result<Response> {
    if !page.can("users.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    user_owner(user_id)
        .clear_media_collection(&ctx, LOGO_COLLECTION)
        .await?;
    loco_activity_log::activity(&ctx.db)
        .in_log("users")
        .caused_by("user", page.user.id)
        .performed_on("user", user_id)
        .event("image_removed")
        .log_quietly("Removed the profile image")
        .await;
    Ok(image_back(user_id, "saved", "image-removed"))
}

/// Sets a new random password and shows it once on the response (never in a URL or the database
/// in plain text). The person is signed out everywhere else and their API tokens are revoked.
#[debug_handler]
async fn user_generate_password(
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    page: AdminPage,
    Path(user_id): Path<i64>,
) -> Result<Response> {
    if !page.can("users.manage") {
        return page.forbidden(&v);
    }
    if !same_origin(&headers) {
        return Ok((StatusCode::FORBIDDEN, "request rejected").into_response());
    }
    let user = users::Entity::find_by_id(user_id)
        .one(&ctx.db)
        .await?
        .ok_or(Error::NotFound)?;
    let password = generate_password();
    let mut active: users::ActiveModel = user.clone().into();
    active.password = Set(loco_rs::hash::hash_password(&password)?);
    active.updated_at = Set(chrono::Utc::now().fixed_offset());
    active.update(&ctx.db).await?;
    loco_activity_log::activity(&ctx.db)
        .in_log("users")
        .caused_by("user", page.user.id)
        .performed_on("user", user_id)
        .event("password_generated")
        .log_quietly("Generated a new password")
        .await;

    let keep_session = if user_id == page.user.id {
        page.user.session_id
    } else {
        0
    };
    session::delete_others(&ctx.db, user_id, keep_session).await?;
    module_auth_api::revoke_api_tokens(&ctx.db, user_id, 0).await?;

    let data = user_show_data(&ctx, &page, &user, json!({ "new_password": password })).await?;
    let body = v.render("admin/user_edit.html", page.data(data))?;
    Ok((
        StatusCode::OK,
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        axum::response::Html(body),
    )
        .into_response())
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new()
        .add("/admin/users/{id}", get(user_show).post(user_update))
        .add("/admin/users/{id}/edit", get(user_edit))
        .add("/admin/users/{id}/image", post(user_image_upload))
        .add("/admin/users/{id}/image/remove", post(user_image_remove))
        .add("/admin/users/{id}/password", post(user_generate_password))]
}
