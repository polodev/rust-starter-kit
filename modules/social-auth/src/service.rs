//! Turns a Google profile into one of our users: a linked account first, then an account with the
//! same (Google-verified) email, otherwise a new account.

use chrono::Utc;
use loco_rs::{hash, Result};
use module_auth::{session::new_token, users};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::{entities::social_accounts, google::GoogleProfile};

pub const PROVIDER: &str = "google";

/// `None` when there is no matching account and `create` is false.
pub async fn find_or_create_user(
    db: &DatabaseConnection,
    profile: &GoogleProfile,
    create: bool,
) -> Result<Option<users::Model>> {
    let identity = format!("{PROVIDER}:{}", profile.subject);

    if let Some(link) = social_accounts::Entity::find()
        .filter(social_accounts::Column::Identity.eq(identity.as_str()))
        .one(db)
        .await?
    {
        if let Some(user) = users::Entity::find_by_id(link.user_id).one(db).await? {
            return Ok(Some(user));
        }
    }

    let existing = users::Entity::find()
        .filter(users::Column::Email.eq(profile.email.as_str()))
        .one(db)
        .await?;
    let user = match existing {
        Some(user) => user,
        None if !create => return Ok(None),
        None => {
            let name = profile.name.clone().unwrap_or_else(|| {
                profile
                    .email
                    .split('@')
                    .next()
                    .unwrap_or("User")
                    .to_string()
            });
            users::ActiveModel {
                pid: Set(Uuid::new_v4()),
                email: Set(profile.email.clone()),
                // Nobody knows this password; the person can ask an admin for one later.
                password: Set(hash::hash_password(&new_token())?),
                api_key: Set(format!("lo-{}", Uuid::new_v4())),
                name: Set(name.chars().take(80).collect()),
                email_verified_at: Set(Some(Utc::now().fixed_offset())),
                ..Default::default()
            }
            .insert(db)
            .await?
        }
    };

    // A racing second sign-in may already have linked it; the unique identity decides.
    let _ = social_accounts::ActiveModel {
        provider: Set(PROVIDER.to_string()),
        identity: Set(identity),
        email: Set(profile.email.clone()),
        user_id: Set(user.id),
        ..Default::default()
    }
    .insert(db)
    .await;
    Ok(Some(user))
}
