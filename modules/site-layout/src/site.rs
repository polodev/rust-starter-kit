//! What every public page needs to render the shared layout: language, current path and the
//! signed-in person, if any. The `auth` module puts a [`SiteUser`] into the request extensions.

use std::convert::Infallible;

use axum::{extract::FromRequestParts, http::request::Parts};
use serde::Serialize;
use serde_json::{json, Value};

use crate::locale::{fluent_tag, Locale};

/// The signed-in person as the header needs to show them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SiteUser {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub initial: String,
    pub avatar_url: Option<String>,
    pub role_name: Option<String>,
    /// Holds `dashboard.view`, so the menu shows the admin dashboard link.
    pub can_view_admin: bool,
    /// The person's permission keys, for staff-only links on public pages (their own, never cached).
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Site {
    pub locale: Locale,
    pub path: String,
    pub user: Option<SiteUser>,
}

impl Site {
    pub fn lang(&self) -> &'static str {
        self.locale.code()
    }

    pub fn tag(&self) -> &'static str {
        fluent_tag(self.locale.code())
    }

    /// Template data: the shared keys (`lang`, `tag`, `path`, `user`) plus the page's own keys.
    pub fn data(&self, extra: Value) -> Value {
        let mut data =
            json!({ "lang": self.lang(), "tag": self.tag(), "path": self.path, "user": self.user });
        if let (Some(base), Value::Object(extra)) = (data.as_object_mut(), extra) {
            base.extend(extra);
        }
        data
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Site {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Site {
            locale: Locale::from_parts(parts),
            path: parts.uri.path().to_string(),
            user: parts.extensions.get::<SiteUser>().cloned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_merges_shared_and_page_keys() {
        let site = Site {
            locale: Locale("bn"),
            path: "/x".to_string(),
            user: None,
        };
        let data = site.data(json!({ "page": "p" }));
        assert_eq!(data["lang"], "bn");
        assert_eq!(data["tag"], "bn-BD");
        assert_eq!(data["path"], "/x");
        assert!(data["user"].is_null());
        assert_eq!(data["page"], "p");
    }
}
