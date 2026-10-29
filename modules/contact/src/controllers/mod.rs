use loco_rs::controller::Routes;

mod admin;
mod public;

pub fn routes() -> Vec<Routes> {
    let mut all = public::routes();
    all.extend(admin::routes());
    all
}

/// True when the request comes from the component script (`X-Component: 1`), which wants just the
/// component's HTML instead of the whole page.
pub(crate) fn wants_fragment(headers: &axum::http::HeaderMap) -> bool {
    headers.contains_key("x-component")
}
