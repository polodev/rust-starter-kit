//! Laravel-shaped JSON replies: `{"message": ...}` for errors and `{"message", "errors": {field: [..]}}`
//! for validation, so a client written for a Laravel API works unchanged.

use std::collections::BTreeMap;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

pub fn message(status: StatusCode, text: &str) -> Response {
    (status, Json(json!({ "message": text }))).into_response()
}

pub fn unauthenticated() -> Response {
    message(StatusCode::UNAUTHORIZED, "Unauthenticated.")
}

/// Field errors collected while validating one request.
#[derive(Default)]
pub struct Errors {
    fields: BTreeMap<&'static str, Vec<String>>,
    first: Option<String>,
}

impl Errors {
    pub fn add(&mut self, field: &'static str, text: impl Into<String>) {
        let text = text.into();
        self.first.get_or_insert_with(|| text.clone());
        self.fields.entry(field).or_default().push(text);
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// 422 with Laravel's summary: the first message plus "(and N more errors)".
    pub fn into_response(self) -> Response {
        let total: usize = self.fields.values().map(Vec::len).sum();
        let first = self
            .first
            .clone()
            .unwrap_or_else(|| "The given data was invalid.".to_string());
        let summary = match total {
            0 | 1 => first,
            2 => format!("{first} (and 1 more error)"),
            more => format!("{first} (and {} more errors)", more - 1),
        };
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "message": summary, "errors": self.fields })),
        )
            .into_response()
    }
}

/// A JSON body read leniently: unreadable or non-object bodies behave like `{}` so the caller gets
/// a normal validation error instead of a framework parse error.
pub struct Input(Value);

impl Input {
    pub fn parse(bytes: &[u8]) -> Self {
        Self(serde_json::from_slice(bytes).unwrap_or(Value::Null))
    }

    /// A string field, trimmed; `None` when missing, empty or not a string.
    pub fn text(&self, field: &str) -> Option<String> {
        self.0
            .get(field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }

    /// A string field kept exactly as sent (passwords).
    pub fn raw(&self, field: &str) -> Option<String> {
        self.0
            .get(field)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_counts_the_other_errors() {
        let mut errors = Errors::default();
        errors.add("email", "The email field is required.");
        assert!(!errors.is_empty());
        errors.add("password", "The password field is required.");
        errors.add("name", "The name field is required.");
        let response = errors.into_response();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn bad_bodies_read_as_empty() {
        assert_eq!(Input::parse(b"not json").text("email"), None);
        assert_eq!(Input::parse(b"[1]").text("email"), None);
        let input = Input::parse(br#"{"email":"  a@b.co ","password":" x "}"#);
        assert_eq!(input.text("email").as_deref(), Some("a@b.co"));
        assert_eq!(input.raw("password").as_deref(), Some(" x "));
    }
}
