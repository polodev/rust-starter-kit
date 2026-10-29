//! Makes the database match `definitions.rs`. Safe to run any number of times.

use loco_rs::{Error, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};

use crate::definitions::{permission_definitions, role_definitions, PermissionDef, RoleDef};
use crate::entities::{permission_role, permissions, roles};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub permissions_created: usize,
    pub permissions_updated: usize,
    pub permissions_deactivated: usize,
    pub roles_created: usize,
    pub roles_updated: usize,
    pub roles_deactivated: usize,
}

/// Keys matched by one grant: an exact key, `area.*`, or `*`.
fn expand(pattern: &str, keys: &[String]) -> Vec<String> {
    if pattern == "*" {
        return keys.to_vec();
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return keys
            .iter()
            .filter(|key| key.starts_with(prefix))
            .cloned()
            .collect();
    }
    keys.iter()
        .filter(|key| key.as_str() == pattern)
        .cloned()
        .collect()
}

/// Effective permission keys per role (lowest rank first): each role keeps everything below it.
pub fn effective_permissions(
    roles: &[RoleDef],
    permissions: &[PermissionDef],
) -> Result<Vec<(RoleDef, Vec<String>)>> {
    let keys: Vec<String> = permissions.iter().map(|p| p.key.clone()).collect();
    let mut seen_keys = std::collections::HashSet::new();
    for key in &keys {
        if !seen_keys.insert(key) {
            return Err(Error::string(&format!("duplicate permission key `{key}`")));
        }
    }

    let mut accumulated: Vec<String> = Vec::new();
    let mut result = Vec::new();
    for (index, role) in roles.iter().enumerate() {
        if role.rank != i32::try_from(index).unwrap_or(i32::MAX) + 1 {
            return Err(Error::string(&format!(
                "role `{}` must have rank {} (roles go lowest first)",
                role.slug,
                index + 1
            )));
        }
        for pattern in role.grants {
            let matched = expand(pattern, &keys);
            if matched.is_empty() {
                return Err(Error::string(&format!(
                    "role `{}` grants `{pattern}`, which matches no permission",
                    role.slug
                )));
            }
            for key in matched {
                if !accumulated.contains(&key) {
                    accumulated.push(key);
                }
            }
        }
        result.push((role.clone(), accumulated.clone()));
    }
    Ok(result)
}

pub async fn sync(db: &DatabaseConnection) -> Result<SyncReport> {
    let permission_defs = permission_definitions();
    let role_defs = effective_permissions(&role_definitions(), &permission_defs)?;
    let mut report = SyncReport::default();

    let txn = db.begin().await?;

    for def in &permission_defs {
        match permissions::Entity::find()
            .filter(permissions::Column::Key.eq(def.key.as_str()))
            .one(&txn)
            .await?
        {
            Some(existing) => {
                let mut active: permissions::ActiveModel = existing.into();
                active.name = Set(def.name.clone());
                active.group_name = Set(def.group.clone());
                active.is_active = Set(true);
                active.update(&txn).await?;
                report.permissions_updated += 1;
            }
            None => {
                permissions::ActiveModel {
                    key: Set(def.key.clone()),
                    name: Set(def.name.clone()),
                    group_name: Set(def.group.clone()),
                    is_active: Set(true),
                    ..Default::default()
                }
                .insert(&txn)
                .await?;
                report.permissions_created += 1;
            }
        }
    }
    let configured: Vec<&str> = permission_defs.iter().map(|d| d.key.as_str()).collect();
    for stale in permissions::Entity::find()
        .filter(permissions::Column::IsActive.eq(true))
        .all(&txn)
        .await?
    {
        if !configured.contains(&stale.key.as_str()) {
            let mut active: permissions::ActiveModel = stale.into();
            active.is_active = Set(false);
            active.update(&txn).await?;
            report.permissions_deactivated += 1;
        }
    }

    for (def, keys) in &role_defs {
        let role = match roles::Entity::find()
            .filter(roles::Column::Slug.eq(def.slug))
            .one(&txn)
            .await?
        {
            Some(existing) => {
                let mut active: roles::ActiveModel = existing.into();
                active.name = Set(def.name.to_string());
                active.rank = Set(def.rank);
                active.description = Set(Some(def.description.to_string()));
                active.is_active = Set(true);
                report.roles_updated += 1;
                active.update(&txn).await?
            }
            None => {
                report.roles_created += 1;
                roles::ActiveModel {
                    slug: Set(def.slug.to_string()),
                    name: Set(def.name.to_string()),
                    rank: Set(def.rank),
                    description: Set(Some(def.description.to_string())),
                    is_active: Set(true),
                    ..Default::default()
                }
                .insert(&txn)
                .await?
            }
        };

        permission_role::Entity::delete_many()
            .filter(permission_role::Column::RoleId.eq(role.id))
            .exec(&txn)
            .await?;
        let permission_ids: Vec<i64> = permissions::Entity::find()
            .filter(permissions::Column::Key.is_in(keys.clone()))
            .all(&txn)
            .await?
            .into_iter()
            .map(|permission| permission.id)
            .collect();
        if !permission_ids.is_empty() {
            permission_role::Entity::insert_many(permission_ids.into_iter().map(|permission_id| {
                permission_role::ActiveModel {
                    role_id: Set(role.id),
                    permission_id: Set(permission_id),
                    ..Default::default()
                }
            }))
            .exec(&txn)
            .await?;
        }
    }
    let slugs: Vec<&str> = role_defs.iter().map(|(def, _)| def.slug).collect();
    for stale in roles::Entity::find()
        .filter(roles::Column::IsActive.eq(true))
        .all(&txn)
        .await?
    {
        if !slugs.contains(&stale.slug.as_str()) {
            let mut active: roles::ActiveModel = stale.into();
            active.is_active = Set(false);
            active.update(&txn).await?;
            report.roles_deactivated += 1;
        }
    }

    txn.commit().await?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_definitions_are_valid_and_cumulative() {
        let effective =
            effective_permissions(&role_definitions(), &permission_definitions()).unwrap();
        assert_eq!(effective.len(), 6);
        for pair in effective.windows(2) {
            assert!(
                pair[0].1.iter().all(|key| pair[1].1.contains(key)),
                "{} must keep all of {}",
                pair[1].0.slug,
                pair[0].0.slug
            );
        }
        assert!(
            effective[0].1.contains(&"dashboard.view".to_string()),
            "the lowest role opens the dashboard"
        );
        let marketing = &effective[2].1;
        assert!(
            marketing.contains(&"pages.manage".to_string())
                && marketing.contains(&"users.view".to_string())
        );
        assert!(!marketing.contains(&"users.manage".to_string()));
        let order: Vec<_> = effective.iter().map(|(role, _)| role.slug).collect();
        assert_eq!(
            order,
            [
                "new_employee",
                "business_development",
                "marketing",
                "employee",
                "admin",
                "developer"
            ]
        );
        let developer = &effective[5].1;
        assert_eq!(
            developer.len(),
            permission_definitions().len(),
            "developer holds every permission"
        );
        for manager_permission in ["roles.view", "roles.assign", "roles.sync"] {
            assert!(developer.contains(&manager_permission.to_string()));
            assert!(
                !effective[4].1.contains(&manager_permission.to_string()),
                "admin must not hold {manager_permission}"
            );
        }
    }

    #[test]
    fn unknown_pattern_is_rejected() {
        let mut roles = role_definitions();
        roles[0].grants = &["nothing.here"];
        assert!(effective_permissions(&roles, &permission_definitions()).is_err());
    }

    #[test]
    fn ranks_must_ascend_from_one() {
        let mut roles = role_definitions();
        roles.swap(0, 1);
        assert!(effective_permissions(&roles, &permission_definitions()).is_err());
    }

    #[test]
    fn patterns_expand() {
        let keys = vec![
            "roles.view".to_string(),
            "roles.sync".to_string(),
            "pages.view".to_string(),
        ];
        assert_eq!(expand("roles.*", &keys), ["roles.view", "roles.sync"]);
        assert_eq!(expand("*", &keys).len(), 3);
        assert_eq!(expand("pages.view", &keys), ["pages.view"]);
    }
}
