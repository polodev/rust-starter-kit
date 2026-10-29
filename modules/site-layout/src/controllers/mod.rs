use axum::{
    extract::{Path, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use loco_rs::prelude::*;
use serde::Deserialize;

use crate::{
    locale::{normalize, set_cookie_value},
    site::Site,
};

#[derive(Deserialize)]
struct LanguageQuery {
    next: Option<String>,
}

#[debug_handler]
async fn home(ViewEngine(v): ViewEngine<TeraView>, site: Site) -> Result<Response> {
    format::render().view(&v, "site/home.html", site.data(serde_json::json!({})))
}

/// Stores the language in a one-year cookie and returns to the page the visitor came from.
async fn set_language(Path(code): Path<String>, Query(query): Query<LanguageQuery>) -> Response {
    let Some(code) = normalize(&code) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // Only same-site absolute paths; `//host` and full URLs would be an open redirect.
    let next = query
        .next
        .filter(|path| path.starts_with('/') && !path.starts_with("//") && !path.contains('\\'))
        .unwrap_or_else(|| "/".to_string());
    (
        StatusCode::SEE_OTHER,
        [
            (header::SET_COOKIE, set_cookie_value(code)),
            (header::LOCATION, next),
        ],
    )
        .into_response()
}

pub fn routes() -> Vec<Routes> {
    vec![Routes::new()
        .add("/", get(home))
        .add("/language/{code}", get(set_language))]
}
