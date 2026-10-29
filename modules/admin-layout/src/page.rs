//! Everything an admin page needs: who is signed in, what they may see, and the sidebar.

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{Html, IntoResponse, Response},
};
use loco_modular::MenuItem;
use loco_rs::{controller::views::ViewRenderer, prelude::TeraView, Result};
use module_auth::{Auth, AuthUser};
use serde::Serialize;
use serde_json::{json, Value};

/// Admin sidebar links collected from every module; the app puts this in the request extensions.
#[derive(Debug, Clone, Default)]
pub struct AdminMenu(pub Vec<MenuItem>);

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct NavLink {
    pub label: &'static str,
    pub href: &'static str,
    pub icon: &'static str,
    pub active: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct NavSection {
    pub title: &'static str,
    pub links: Vec<NavLink>,
}

pub struct AdminPage {
    pub user: AuthUser,
    pub path: String,
    pub nav: Vec<NavSection>,
    /// The person's profile image, when they have one (added by the accounts module).
    pub avatar_url: Option<String>,
}

impl AdminPage {
    pub fn can(&self, permission: &str) -> bool {
        self.user.access.can(permission)
    }

    /// Template data: the shell (`user`, `nav`, `nav_json`, `path`) plus the page's own keys.
    pub fn data(&self, extra: Value) -> Value {
        let mut data = json!({
            "user": {
                "name": self.user.name,
                "email": self.user.email,
                "initial": self.user.name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
                "avatar_url": self.avatar_url,
                "role": self.user.access.role.as_ref().map(|role| role.name.clone()),
            },
            "nav": self.nav,
            "nav_json": serde_json::to_string(&self.nav).unwrap_or_else(|_| "[]".to_string()),
            "path": self.path,
        });
        if let (Some(base), Value::Object(extra)) = (data.as_object_mut(), extra) {
            base.extend(extra);
        }
        data
    }

    pub fn render(&self, v: &TeraView, template: &str, extra: Value) -> Result<Response> {
        Ok(Html(v.render(template, self.data(extra))?).into_response())
    }

    /// The 403 page, shown inside the admin shell.
    pub fn forbidden(&self, v: &TeraView) -> Result<Response> {
        let body = v.render(
            "admin/forbidden.html",
            self.data(json!({ "title": "Access denied" })),
        )?;
        Ok((StatusCode::FORBIDDEN, Html(body)).into_response())
    }
}

/// Sections and links this person may see, with the current page marked active.
pub fn build_nav(items: &[MenuItem], user: &AuthUser, path: &str) -> Vec<NavSection> {
    let mut sections: Vec<NavSection> = Vec::new();
    for item in items.iter().filter(|item| user.access.can(item.permission)) {
        let active = path == item.href
            || (item.match_children && path.starts_with(&format!("{}/", item.href)));
        let link = NavLink {
            label: item.label,
            href: item.href,
            icon: item.icon,
            active,
        };
        match sections.last_mut() {
            Some(section) if section.title == item.section => section.links.push(link),
            _ => sections.push(NavSection {
                title: item.section,
                links: vec![link],
            }),
        }
    }
    sections
}

impl<S: Send + Sync> FromRequestParts<S> for AdminPage {
    type Rejection = axum::response::Redirect;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Auth(user) = Auth::from_request_parts(parts, state).await?;
        let menu = parts
            .extensions
            .get::<AdminMenu>()
            .cloned()
            .unwrap_or_default();
        let path = parts.uri.path().to_string();
        let nav = build_nav(&menu.0, &user, &path);
        let avatar_url = parts
            .extensions
            .get::<module_site_layout::SiteUser>()
            .and_then(|site_user| site_user.avatar_url.clone());
        Ok(AdminPage {
            user,
            path,
            nav,
            avatar_url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use module_role_permission::Access;

    fn user(permissions: &[&str]) -> AuthUser {
        AuthUser {
            id: 1,
            session_id: 1,
            name: "Ada".into(),
            email: "ada@example.com".into(),
            created_at: chrono::Utc::now().fixed_offset(),
            access: Access {
                role: None,
                permissions: permissions.iter().map(|p| (*p).to_string()).collect(),
            },
        }
    }

    fn item(
        section: &'static str,
        label: &'static str,
        href: &'static str,
        permission: &'static str,
        children: bool,
    ) -> MenuItem {
        MenuItem {
            section,
            label,
            href,
            permission,
            icon: "fa-solid fa-circle",
            order: 1,
            match_children: children,
        }
    }

    #[test]
    fn nav_only_contains_links_the_person_may_use() {
        let items = [
            item("Overview", "Dashboard", "/admin", "dashboard.view", false),
            item("People", "Users", "/admin/users", "users.view", true),
        ];
        let nav = build_nav(&items, &user(&["dashboard.view"]), "/admin");
        assert_eq!(nav.len(), 1);
        assert_eq!(nav[0].links.len(), 1);
        assert!(nav[0].links[0].active);

        let nav = build_nav(&items, &user(&[]), "/admin");
        assert!(nav.is_empty());
    }

    #[test]
    fn child_paths_highlight_their_parent_link() {
        let items = [
            item("People", "Users", "/admin/users", "users.view", true),
            item("People", "Roles", "/admin/roles", "roles.view", false),
        ];
        let nav = build_nav(
            &items,
            &user(&["users.view", "roles.view"]),
            "/admin/users/7",
        );
        assert!(nav[0].links[0].active);
        assert!(!nav[0].links[1].active);
    }
}
