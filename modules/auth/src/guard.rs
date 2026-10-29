//! Who is asking, and whether a form post really came from this site.

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderMap},
    response::Redirect,
};
use module_role_permission::Access;

/// The signed-in person for the current request.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i64,
    pub session_id: i64,
    pub name: String,
    pub email: String,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub access: Access,
}

/// Requires a signed-in person; otherwise redirects to the login page and returns afterwards.
pub struct Auth(pub AuthUser);

/// The signed-in person if there is one.
pub struct OptionalAuth(pub Option<AuthUser>);

impl<S: Send + Sync> FromRequestParts<S> for Auth {
    type Rejection = Redirect;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        match parts.extensions.get::<AuthUser>() {
            Some(user) => Ok(Auth(user.clone())),
            None => {
                let next = parts
                    .uri
                    .path_and_query()
                    .map(|path| path.as_str().to_string())
                    .unwrap_or_else(|| "/".to_string());
                Err(Redirect::to(&format!(
                    "/login?next={}",
                    query_escape(&next)
                )))
            }
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for OptionalAuth {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(OptionalAuth(parts.extensions.get::<AuthUser>().cloned()))
    }
}

fn query_escape(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// A same-site absolute path, or `/`. Blocks `//host`, full URLs and the login/register pages.
pub fn safe_next(next: Option<&str>) -> String {
    match next {
        Some(path)
            if path.starts_with('/')
                && !path.starts_with("//")
                && !path.contains('\\')
                && !path.starts_with("/login")
                && !path.starts_with("/register") =>
        {
            path.to_string()
        }
        _ => "/".to_string(),
    }
}

fn host_of(url: &str) -> Option<&str> {
    url.split_once("://")?.1.split('/').next()
}

/// CSRF defence for form posts: the `Origin` (or else `Referer`) host must be this site.
/// Together with `SameSite=Lax` cookies this stops other sites from posting as the visitor.
pub fn same_origin(headers: &HeaderMap) -> bool {
    fn text(headers: &HeaderMap, name: impl axum::http::header::AsHeaderName) -> Option<&str> {
        headers.get(name).and_then(|value| value.to_str().ok())
    }
    let Some(host) = text(headers, header::HOST) else {
        return false;
    };
    let forwarded = text(headers, "x-forwarded-host");
    let matches = |candidate: &str| {
        candidate.eq_ignore_ascii_case(host)
            || forwarded.is_some_and(|f| candidate.eq_ignore_ascii_case(f))
    };

    if let Some(origin) = text(headers, header::ORIGIN) {
        return host_of(origin).is_some_and(matches);
    }
    text(headers, header::REFERER)
        .and_then(host_of)
        .is_some_and(matches)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, value.parse().unwrap());
        }
        map
    }

    #[test]
    fn next_must_be_a_local_path() {
        assert_eq!(safe_next(Some("/pages/about")), "/pages/about");
        for bad in [
            "//evil.example",
            "https://evil.example",
            "/\\evil",
            "/login",
            "/register?x=1",
            "",
        ] {
            assert_eq!(safe_next(Some(bad)), "/", "{bad}");
        }
        assert_eq!(safe_next(None), "/");
    }

    #[test]
    fn same_origin_checks_origin_then_referer() {
        assert!(same_origin(&headers(&[
            ("host", "localhost:5150"),
            ("origin", "http://localhost:5150")
        ])));
        assert!(!same_origin(&headers(&[
            ("host", "localhost:5150"),
            ("origin", "https://evil.example")
        ])));
        assert!(!same_origin(&headers(&[
            ("host", "localhost:5150"),
            ("origin", "null")
        ])));
        assert!(same_origin(&headers(&[
            ("host", "a.test"),
            ("referer", "https://a.test/login")
        ])));
        assert!(
            !same_origin(&headers(&[("host", "a.test")])),
            "no Origin and no Referer is rejected"
        );
    }

    #[test]
    fn query_escape_keeps_paths_readable() {
        assert_eq!(query_escape("/a/b?x=1&y=2"), "/a/b%3Fx%3D1%26y%3D2");
    }
}
