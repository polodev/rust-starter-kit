//! Roles and permissions are defined here, in code. Run `roles:sync` after editing.

/// Areas that get a `view` and a `manage` permission each: `(area, what it covers)`.
const AREAS: [(&str, &str); 7] = [
    ("dashboard", "admin dashboard"),
    ("users", "user accounts"),
    ("pages", "website pages"),
    ("media", "uploaded files"),
    ("settings", "site settings"),
    ("logs", "activity and API logs"),
    ("queue", "jobs and schedule"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDef {
    pub key: String,
    pub name: String,
    pub group: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleDef {
    pub slug: &'static str,
    pub name: &'static str,
    /// 1 is the lowest role. Every role includes the permissions of all roles below it.
    pub rank: i32,
    pub description: &'static str,
    /// Permission keys or patterns (`pages.*`, `*`) granted by this role itself, on top of the lower roles.
    pub grants: &'static [&'static str],
}

pub fn permission_definitions() -> Vec<PermissionDef> {
    let mut defs = Vec::new();
    for (area, label) in AREAS {
        defs.push(PermissionDef {
            key: format!("{area}.view"),
            name: format!("View {label}"),
            group: area.to_string(),
        });
        defs.push(PermissionDef {
            key: format!("{area}.manage"),
            name: format!("Manage {label}"),
            group: area.to_string(),
        });
    }
    for (key, name) in [
        ("roles.view", "View role and permission definitions"),
        ("roles.assign", "Assign roles to users"),
        ("roles.sync", "Sync config-defined roles and permissions"),
    ] {
        defs.push(PermissionDef {
            key: key.to_string(),
            name: name.to_string(),
            group: "roles".to_string(),
        });
    }
    defs
}

/// Platform roles, lowest first. Each role keeps every permission of the roles below it:
/// new_employee < business_development < marketing < employee < admin < developer.
pub fn role_definitions() -> Vec<RoleDef> {
    vec![
        RoleDef {
            slug: "new_employee",
            name: "New Employee",
            rank: 1,
            description: "Admin dashboard overview only.",
            grants: &["dashboard.view"],
        },
        RoleDef {
            slug: "business_development",
            name: "Business Development",
            rank: 2,
            description: "New employee plus a look at users and website pages.",
            grants: &["users.view", "pages.view"],
        },
        RoleDef {
            slug: "marketing",
            name: "Marketing",
            rank: 3,
            description:
                "Business development plus editing website pages and viewing uploaded files.",
            grants: &["pages.manage", "media.view"],
        },
        RoleDef {
            slug: "employee",
            name: "Employee",
            rank: 4,
            description: "Marketing plus read access to every operational area.",
            grants: &["settings.view", "logs.view", "queue.view"],
        },
        RoleDef {
            slug: "admin",
            name: "Admin",
            rank: 5,
            description: "Manages the platform but cannot use the role-permission manager.",
            grants: &[
                "dashboard.manage",
                "users.manage",
                "media.manage",
                "settings.manage",
                "logs.manage",
                "queue.manage",
            ],
        },
        RoleDef {
            slug: "developer",
            name: "Developer",
            rank: 6,
            description:
                "Every permission, including the role-permission manager (view, assign, sync).",
            grants: &["*"],
        },
    ]
}
