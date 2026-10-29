//! Cookie sessions stored in the database. The cookie holds a random token; the table holds only
//! its SHA-256, so a database leak cannot be replayed as a login.

use chrono::{Duration, Utc};
use loco_rs::Result;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entities::{sessions, users};

pub const COOKIE: &str = "session";
pub const LIFETIME_DAYS: i64 = 30;
/// A session that is used keeps sliding forward, but the database is written at most this often.
const REFRESH_AFTER_MINUTES: i64 = 60;

/// 256 bits of randomness from two v4 UUIDs.
pub fn new_token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

pub fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

pub fn cookie_header(token: &str, secure: bool) -> String {
    let max_age = LIFETIME_DAYS * 24 * 60 * 60;
    let secure = if secure { "; Secure" } else { "" };
    format!("{COOKIE}={token}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}")
}

pub fn clearing_cookie_header(secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{COOKIE}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax{secure}")
}

pub fn token_from_cookie_header(header: Option<&str>) -> Option<&str> {
    header?
        .split(';')
        .find_map(|cookie| cookie.trim().strip_prefix("session="))
        .filter(|token| token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Starts a session and returns the plain token to put in the cookie.
pub async fn create(
    db: &DatabaseConnection,
    user_id: i64,
    user_agent: Option<&str>,
) -> Result<String> {
    let token = new_token();
    let now = Utc::now().fixed_offset();
    sessions::ActiveModel {
        token_hash: Set(hash_token(&token)),
        user_agent: Set(user_agent.map(|agent| agent.chars().take(255).collect())),
        expires_at: Set(now + Duration::days(LIFETIME_DAYS)),
        last_used_at: Set(now),
        user_id: Set(user_id),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(token)
}

/// The live session and its user for this cookie token, or `None` when unknown or expired.
pub async fn find(
    db: &DatabaseConnection,
    token: &str,
) -> Result<Option<(sessions::Model, users::Model)>> {
    let Some(session) = sessions::Entity::find()
        .filter(sessions::Column::TokenHash.eq(hash_token(token)))
        .filter(sessions::Column::ExpiresAt.gt(Utc::now().fixed_offset()))
        .one(db)
        .await?
    else {
        return Ok(None);
    };
    let Some(user) = users::Entity::find_by_id(session.user_id).one(db).await? else {
        return Ok(None);
    };
    Ok(Some((session, user)))
}

/// Slides the expiry forward, at most once per hour.
pub async fn touch(db: &DatabaseConnection, session: sessions::Model) -> Result<()> {
    let now = Utc::now().fixed_offset();
    if now - session.last_used_at < Duration::minutes(REFRESH_AFTER_MINUTES) {
        return Ok(());
    }
    let mut active: sessions::ActiveModel = session.into();
    active.last_used_at = Set(now);
    active.expires_at = Set(now + Duration::days(LIFETIME_DAYS));
    active.update(db).await?;
    Ok(())
}

pub async fn delete_by_token(db: &DatabaseConnection, token: &str) -> Result<()> {
    sessions::Entity::delete_many()
        .filter(sessions::Column::TokenHash.eq(hash_token(token)))
        .exec(db)
        .await?;
    Ok(())
}

/// Signs the person out everywhere except the session with id `keep_id`.
pub async fn delete_others(db: &DatabaseConnection, user_id: i64, keep_id: i64) -> Result<u64> {
    let result = sessions::Entity::delete_many()
        .filter(sessions::Column::UserId.eq(user_id))
        .filter(sessions::Column::Id.ne(keep_id))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}

/// Removes expired sessions; returns how many.
pub async fn prune(db: &DatabaseConnection) -> Result<u64> {
    let result = sessions::Entity::delete_many()
        .filter(sessions::Column::ExpiresAt.lt(Utc::now().fixed_offset()))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_64_hex_and_unique() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn stored_hash_is_not_the_token() {
        let token = new_token();
        let hash = hash_token(&token);
        assert_eq!(hash.len(), 64);
        assert_ne!(hash, token);
        assert_eq!(hash, hash_token(&token));
    }

    #[test]
    fn cookie_is_http_only_and_lax() {
        let cookie = cookie_header("abc", false);
        assert!(
            cookie.contains("HttpOnly")
                && cookie.contains("SameSite=Lax")
                && !cookie.contains("Secure")
        );
        assert!(cookie_header("abc", true).contains("Secure"));
        assert!(clearing_cookie_header(false).contains("Max-Age=0"));
    }

    #[test]
    fn malformed_cookie_tokens_are_ignored() {
        let good = "a".repeat(64);
        assert_eq!(
            token_from_cookie_header(Some(&format!("x=1; session={good}"))),
            Some(good.as_str())
        );
        assert_eq!(token_from_cookie_header(Some("session=short")), None);
        assert_eq!(
            token_from_cookie_header(Some(&format!("session={}", "z".repeat(64)))),
            None
        );
        assert_eq!(token_from_cookie_header(None), None);
    }
}
