//! Personal access tokens for API clients (the Laravel Sanctum idea). A token looks like
//! `{id}|{secret}`; the table keeps only the SHA-256 of the secret, so a leaked table cannot be
//! replayed. Unlike a web session, a token has a name and lives until it is revoked or expires.

use chrono::{DateTime, Duration, FixedOffset, Utc};
use loco_rs::Result;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use module_auth::{
    session::{hash_token, new_token},
    users,
};

use crate::entities::personal_access_tokens as tokens;

/// A token that is used keeps its `last_used_at`, but the database is written at most this often.
const TOUCH_AFTER_MINUTES: i64 = 5;

/// Creates a token and returns `(plain text token, stored row)`. The plain text is shown once.
pub async fn create(
    db: &DatabaseConnection,
    user_id: i64,
    name: &str,
    lifetime: Option<Duration>,
) -> Result<(String, tokens::Model)> {
    let secret = new_token();
    let row = tokens::ActiveModel {
        name: Set(name.chars().take(100).collect()),
        token_hash: Set(hash_token(&secret)),
        expires_at: Set(lifetime.map(|lifetime| Utc::now().fixed_offset() + lifetime)),
        user_id: Set(user_id),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok((format!("{}|{secret}", row.id), row))
}

/// Splits `id|secret`, rejecting anything that is not shaped like a token.
pub fn parse(plain: &str) -> Option<(i64, &str)> {
    let (id, secret) = plain.split_once('|')?;
    let valid = secret.len() == 64 && secret.bytes().all(|byte| byte.is_ascii_hexdigit());
    valid
        .then(|| id.parse().ok())
        .flatten()
        .map(|id| (id, secret))
}

/// The token row and its user for a plain text token, or `None` when unknown, wrong or expired.
pub async fn find(
    db: &DatabaseConnection,
    plain: &str,
) -> Result<Option<(tokens::Model, users::Model)>> {
    let Some((id, secret)) = parse(plain) else {
        return Ok(None);
    };
    let Some(token) = tokens::Entity::find_by_id(id)
        .filter(tokens::Column::TokenHash.eq(hash_token(secret)))
        .one(db)
        .await?
    else {
        return Ok(None);
    };
    if token
        .expires_at
        .is_some_and(|expires_at| expires_at <= Utc::now().fixed_offset())
    {
        return Ok(None);
    }
    let Some(user) = users::Entity::find_by_id(token.user_id).one(db).await? else {
        return Ok(None);
    };
    Ok(Some((token, user)))
}

pub async fn touch(db: &DatabaseConnection, token: tokens::Model) -> Result<()> {
    let now: DateTime<FixedOffset> = Utc::now().fixed_offset();
    if token
        .last_used_at
        .is_some_and(|last| now - last < Duration::minutes(TOUCH_AFTER_MINUTES))
    {
        return Ok(());
    }
    let mut active: tokens::ActiveModel = token.into();
    active.last_used_at = Set(Some(now));
    active.update(db).await?;
    Ok(())
}

pub async fn for_user(db: &DatabaseConnection, user_id: i64) -> Result<Vec<tokens::Model>> {
    Ok(tokens::Entity::find()
        .filter(tokens::Column::UserId.eq(user_id))
        .all(db)
        .await?)
}

/// Revokes one token of this user; false when it is not theirs or does not exist.
pub async fn revoke(db: &DatabaseConnection, user_id: i64, id: i64) -> Result<bool> {
    let result = tokens::Entity::delete_many()
        .filter(tokens::Column::UserId.eq(user_id))
        .filter(tokens::Column::Id.eq(id))
        .exec(db)
        .await?;
    Ok(result.rows_affected > 0)
}

/// Revokes every token of this user except `keep_id` (pass 0 to revoke all).
pub async fn revoke_others(db: &DatabaseConnection, user_id: i64, keep_id: i64) -> Result<u64> {
    let result = tokens::Entity::delete_many()
        .filter(tokens::Column::UserId.eq(user_id))
        .filter(tokens::Column::Id.ne(keep_id))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}

/// Removes expired tokens; returns how many.
pub async fn prune(db: &DatabaseConnection) -> Result<u64> {
    let result = tokens::Entity::delete_many()
        .filter(tokens::Column::ExpiresAt.lt(Utc::now().fixed_offset()))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_well_formed_tokens_parse() {
        let secret = "a".repeat(64);
        assert_eq!(parse(&format!("12|{secret}")), Some((12, secret.as_str())));
        for bad in ["", "12", "x|".to_string().as_str(), "12|short", "ab|aaaa"] {
            assert_eq!(parse(bad), None, "{bad}");
        }
        assert_eq!(parse(&format!("x|{secret}")), None);
        assert_eq!(parse(&format!("1|{}", "z".repeat(64))), None);
    }
}
