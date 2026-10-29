//! Runs on every request: turns the session cookie into a signed-in person.

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use loco_rs::app::AppContext;
use module_role_permission::{access_for, DASHBOARD_VIEW};
use module_site_layout::SiteUser;

use crate::{guard::AuthUser, session};

pub async fn load_session(
    State(ctx): State<AppContext>,
    mut request: Request,
    next: Next,
) -> Response {
    // Static files never need a person; skip the database for them.
    if !request.uri().path().starts_with("/static") {
        let cookie = request
            .headers()
            .get(axum::http::header::COOKIE)
            .and_then(|value| value.to_str().ok());
        if let Some(token) = session::token_from_cookie_header(cookie).map(str::to_owned) {
            match session::find(&ctx.db, &token).await {
                Ok(Some((live, user))) => {
                    let access = access_for(&ctx.db, user.id).await.unwrap_or_default();
                    request.extensions_mut().insert(SiteUser {
                        id: user.id,
                        initial: user
                            .name
                            .chars()
                            .next()
                            .map(|c| c.to_uppercase().to_string())
                            .unwrap_or_else(|| "?".to_string()),
                        name: user.name.clone(),
                        email: user.email.clone(),
                        avatar_url: None,
                        role_name: access.role.as_ref().map(|role| role.name.clone()),
                        can_view_admin: access.can(DASHBOARD_VIEW),
                    });
                    request.extensions_mut().insert(AuthUser {
                        id: user.id,
                        session_id: live.id,
                        name: user.name,
                        email: user.email,
                        created_at: user.created_at,
                        access,
                    });
                    if let Err(error) = session::touch(&ctx.db, live).await {
                        tracing::warn!(%error, "could not refresh the session");
                    }
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(%error, "session lookup failed"),
            }
        }
    }
    next.run(request).await
}
