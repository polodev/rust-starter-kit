use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use loco_rs::app::AppContext;
use module_media::{CollectionDef, InteractsWithMedia, Owner, DEFAULT_MAX_BYTES, IMAGES};
use module_site_layout::SiteUser;

pub const LOGO_COLLECTION: &str = "logo";
/// 1 MiB, matching the hint shown on the upload form.
const LOGO_MAX_BYTES: usize = 1024 * 1024;

/// The logo is a single-file collection: uploading a new one replaces the old.
pub const fn logo_collection() -> CollectionDef {
    CollectionDef::single(LOGO_COLLECTION)
        .accepts_mime_types(IMAGES)
        .max_bytes(if LOGO_MAX_BYTES < DEFAULT_MAX_BYTES {
            LOGO_MAX_BYTES
        } else {
            DEFAULT_MAX_BYTES
        })
}

pub fn user_owner(user_id: i64) -> Owner {
    Owner::new("user", user_id).with_collections(vec![logo_collection()])
}

pub async fn attach_avatar(
    State(ctx): State<AppContext>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some(site_user) = request.extensions().get::<SiteUser>().cloned() {
        match user_owner(site_user.id)
            .get_first_media_url(&ctx, LOGO_COLLECTION)
            .await
        {
            Ok(Some(url)) => {
                request.extensions_mut().insert(SiteUser {
                    avatar_url: Some(url),
                    ..site_user
                });
            }
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "could not load the account logo"),
        }
    }
    next.run(request).await
}
