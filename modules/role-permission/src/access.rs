//! Who may do what. One role per person plus optional direct permission exceptions.

use std::collections::BTreeSet;

use axum::http::StatusCode;
use loco_rs::{controller::ErrorDetail, Error, Result};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::entities::{permission_role, permission_user, permissions, role_user, roles};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleInfo {
    pub slug: String,
    pub name: String,
    pub rank: i32,
}

/// The role and effective permission keys of one person.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Access {
    pub role: Option<RoleInfo>,
    pub permissions: BTreeSet<String>,
}

impl Access {
    /// True for an exact key, or when a granted `area.*` covers it.
    pub fn can(&self, permission: &str) -> bool {
        self.permissions.iter().any(|granted| {
            granted == permission
                || granted
                    .strip_suffix('*')
                    .is_some_and(|prefix| permission.starts_with(prefix))
        })
    }

    pub fn has_role(&self, slug: &str) -> bool {
        self.role.as_ref().is_some_and(|role| role.slug == slug)
    }

    /// `Ok` when allowed, otherwise a 403 response error.
    pub fn require(&self, permission: &str) -> Result<()> {
        if self.can(permission) {
            Ok(())
        } else {
            Err(forbidden())
        }
    }
}

pub fn forbidden() -> Error {
    Error::CustomError(
        StatusCode::FORBIDDEN,
        ErrorDetail::new("forbidden", "You do not have permission to do this."),
    )
}

pub async fn access_for(db: &DatabaseConnection, user_id: i64) -> Result<Access> {
    let mut access = Access::default();
    let mut permission_ids: Vec<i64> = Vec::new();

    if let Some(link) = role_user::Entity::find()
        .filter(role_user::Column::UserId.eq(user_id))
        .one(db)
        .await?
    {
        if let Some(role) = roles::Entity::find_by_id(link.role_id)
            .filter(roles::Column::IsActive.eq(true))
            .one(db)
            .await?
        {
            permission_ids.extend(
                permission_role::Entity::find()
                    .filter(permission_role::Column::RoleId.eq(role.id))
                    .all(db)
                    .await?
                    .into_iter()
                    .map(|row| row.permission_id),
            );
            access.role = Some(RoleInfo {
                slug: role.slug,
                name: role.name,
                rank: role.rank,
            });
        }
    }
    permission_ids.extend(
        permission_user::Entity::find()
            .filter(permission_user::Column::UserId.eq(user_id))
            .all(db)
            .await?
            .into_iter()
            .map(|row| row.permission_id),
    );

    if !permission_ids.is_empty() {
        access.permissions = permissions::Entity::find()
            .filter(permissions::Column::Id.is_in(permission_ids))
            .filter(permissions::Column::IsActive.eq(true))
            .all(db)
            .await?
            .into_iter()
            .map(|permission| permission.key)
            .collect();
    }
    Ok(access)
}

/// Gives the person exactly this role, replacing any previous one. Returns the role name.
pub async fn assign_role(db: &DatabaseConnection, user_id: i64, slug: &str) -> Result<String> {
    let role = roles::Entity::find()
        .filter(roles::Column::Slug.eq(slug))
        .filter(roles::Column::IsActive.eq(true))
        .one(db)
        .await?
        .ok_or_else(|| {
            Error::string(&format!(
                "unknown role `{slug}`; run roles:sync first or check roles:list"
            ))
        })?;

    role_user::Entity::delete_many()
        .filter(role_user::Column::UserId.eq(user_id))
        .exec(db)
        .await?;
    role_user::ActiveModel {
        user_id: Set(user_id),
        role_id: Set(role.id),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(role.name)
}

pub async fn revoke_role(db: &DatabaseConnection, user_id: i64) -> Result<bool> {
    let removed = role_user::Entity::delete_many()
        .filter(role_user::Column::UserId.eq(user_id))
        .exec(db)
        .await?;
    Ok(removed.rows_affected > 0)
}

/// Direct permission exception. Role-management permissions can only come from a role.
pub async fn grant_permission(db: &DatabaseConnection, user_id: i64, key: &str) -> Result<()> {
    if key.starts_with("roles.") {
        return Err(Error::string(
            "role-management permissions cannot be granted directly; assign a role instead",
        ));
    }
    let permission = permissions::Entity::find()
        .filter(permissions::Column::Key.eq(key))
        .filter(permissions::Column::IsActive.eq(true))
        .one(db)
        .await?
        .ok_or_else(|| Error::string(&format!("unknown permission `{key}`")))?;
    let exists = permission_user::Entity::find_by_id((user_id, permission.id))
        .one(db)
        .await?
        .is_some();
    if !exists {
        permission_user::ActiveModel {
            user_id: Set(user_id),
            permission_id: Set(permission.id),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(())
}

pub async fn revoke_permission(db: &DatabaseConnection, user_id: i64, key: &str) -> Result<bool> {
    let Some(permission) = permissions::Entity::find()
        .filter(permissions::Column::Key.eq(key))
        .one(db)
        .await?
    else {
        return Ok(false);
    };
    let removed = permission_user::Entity::delete_many()
        .filter(permission_user::Column::UserId.eq(user_id))
        .filter(permission_user::Column::PermissionId.eq(permission.id))
        .exec(db)
        .await?;
    Ok(removed.rows_affected > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn access(keys: &[&str]) -> Access {
        Access {
            role: None,
            permissions: keys.iter().map(|k| (*k).to_string()).collect(),
        }
    }

    #[test]
    fn exact_and_wildcard_permissions() {
        let a = access(&["dashboard.view", "pages.*"]);
        assert!(a.can("dashboard.view"));
        assert!(a.can("pages.manage"));
        assert!(!a.can("dashboard.manage"));
        assert!(!a.can("users.view"));
    }

    #[test]
    fn require_returns_forbidden() {
        assert!(access(&[]).require("dashboard.view").is_err());
        assert!(access(&["dashboard.view"])
            .require("dashboard.view")
            .is_ok());
    }
}
