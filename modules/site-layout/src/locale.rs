//! Language selection. A visitor's choice lives in a long-lived `lang` cookie so it survives
//! restarts of the browser, like the login session does.

use std::convert::Infallible;

use axum::{extract::FromRequestParts, http::request::Parts};

pub const SUPPORTED_LOCALES: [&str; 2] = ["en", "bn"];
pub const DEFAULT_LOCALE: &str = "en";
pub const COOKIE_NAME: &str = "lang";
/// One year, in seconds.
pub const COOKIE_MAX_AGE: u64 = 31_536_000;

/// The visitor's language: `?lang=` first, then the `lang` cookie, then `Accept-Language`, then English.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Locale(pub &'static str);

impl Locale {
    pub fn code(self) -> &'static str {
        self.0
    }
}

/// Fluent locale tag used by the `t()` template function.
pub fn fluent_tag(code: &str) -> &'static str {
    match code {
        "bn" => "bn-BD",
        _ => "en-US",
    }
}

pub fn normalize(code: &str) -> Option<&'static str> {
    let primary = code.trim().split(['-', '_']).next()?.to_ascii_lowercase();
    SUPPORTED_LOCALES
        .into_iter()
        .find(|supported| *supported == primary)
}

pub fn from_query(query: Option<&str>) -> Option<&'static str> {
    query?
        .split('&')
        .find_map(|pair| pair.strip_prefix("lang="))
        .and_then(normalize)
}

pub fn from_cookie_header(header: Option<&str>) -> Option<&'static str> {
    header?
        .split(';')
        .find_map(|cookie| cookie.trim().strip_prefix("lang="))
        .and_then(normalize)
}

pub fn from_accept_language(header: Option<&str>) -> Option<&'static str> {
    let mut candidates: Vec<(&str, f32)> = header?
        .split(',')
        .filter_map(|part| {
            let mut pieces = part.trim().split(';');
            let tag = pieces.next()?.trim();
            let quality = pieces
                .find_map(|piece| piece.trim().strip_prefix("q="))
                .and_then(|q| q.parse::<f32>().ok())
                .unwrap_or(1.0);
            Some((tag, quality))
        })
        .collect();
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1));
    candidates.into_iter().find_map(|(tag, _)| normalize(tag))
}

pub fn set_cookie_value(code: &str) -> String {
    format!("{COOKIE_NAME}={code}; Path=/; Max-Age={COOKIE_MAX_AGE}; SameSite=Lax")
}

impl Locale {
    /// Picks the language for a request: `?lang=`, cookie, `Accept-Language`, then the default.
    pub fn from_parts(parts: &Parts) -> Self {
        let header = |name| {
            parts
                .headers
                .get(name)
                .and_then(|value| value.to_str().ok())
        };
        let code = from_query(parts.uri.query())
            .or_else(|| from_cookie_header(header(axum::http::header::COOKIE)))
            .or_else(|| from_accept_language(header(axum::http::header::ACCEPT_LANGUAGE)))
            .unwrap_or(DEFAULT_LOCALE);
        Locale(code)
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Locale {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Locale::from_parts(parts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_wins_over_cookie_and_header() {
        assert_eq!(from_query(Some("a=1&lang=bn")), Some("bn"));
        assert_eq!(from_query(Some("lang=fr")), None);
    }

    #[test]
    fn cookie_is_parsed_among_others() {
        assert_eq!(from_cookie_header(Some("a=b; lang=bn; c=d")), Some("bn"));
        assert_eq!(from_cookie_header(Some("a=b")), None);
    }

    #[test]
    fn accept_language_uses_quality_order() {
        assert_eq!(
            from_accept_language(Some("fr;q=0.9, bn-BD;q=0.8, en;q=0.7")),
            Some("bn")
        );
        assert_eq!(from_accept_language(Some("en-US,bn;q=0.5")), Some("en"));
        assert_eq!(from_accept_language(Some("fr")), None);
    }

    #[test]
    fn fluent_tags_map_codes() {
        assert_eq!(fluent_tag("bn"), "bn-BD");
        assert_eq!(fluent_tag("en"), "en-US");
    }
}
