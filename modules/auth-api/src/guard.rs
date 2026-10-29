//! `ApiAuth`: the Sanctum `auth:sanctum` middleware as an extractor. Put it in a handler's
//! arguments and the handler only runs for a valid `Authorization: Bearer {id}|{secret}` token;
//! anyone else gets `401 {"message": "Unauthenticated."}`.

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
    response::Response,
};
use loco_rs::app::AppContext;
use module_role_permission::{access_for, Access};

use crate::{api_token, json};

/// The person behind the bearer token of this request.
#[derive(Debug, Clone)]
pub struct ApiUser {
    pub id: i64,
    pub token_id: i64,
    pub token_name: String,
    pub name: String,
    pub email: String,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub access: Access,
}

pub struct ApiAuth(pub ApiUser);

pub fn bearer(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

impl FromRequestParts<AppContext> for ApiAuth {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        ctx: &AppContext,
    ) -> Result<Self, Self::Rejection> {
        let plain = bearer(parts).ok_or_else(json::unauthenticated)?;
        let found = api_token::find(&ctx.db, plain).await.map_err(|error| {
            tracing::warn!(%error, "api token lookup failed");
            json::unauthenticated()
        })?;
        let (token, user) = found.ok_or_else(json::unauthenticated)?;
        let access = access_for(&ctx.db, user.id).await.unwrap_or_default();
        let api_user = ApiUser {
            id: user.id,
            token_id: token.id,
            token_name: token.name.clone(),
            name: user.name,
            email: user.email,
            created_at: user.created_at,
            access,
        };
        if let Err(error) = api_token::touch(&ctx.db, token).await {
            tracing::warn!(%error, "could not record api token use");
        }
        Ok(ApiAuth(api_user))
    }
}
