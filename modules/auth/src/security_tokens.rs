//! One-time links sent by email. The link carries a random secret; the table keeps only its SHA-256, a link works
//! once, and it expires. A newer link of the same purpose replaces the older one.

use chrono::{Duration, Utc};
use loco_rs::Result;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};

use crate::{entities::user_security_tokens, session};

pub const EMAIL_VERIFICATION: &str = "email_verification";
pub const PASSWORD_RESET: &str = "password_reset";
pub const MAGIC_LOGIN: &str = "magic_login";

/// How long a link of this purpose works, in minutes.
pub fn lifetime_minutes(purpose: &str) -> i64 {
    match purpose {
        EMAIL_VERIFICATION => 24 * 60,
        PASSWORD_RESET => 60,
        _ => 15,
    }
}

/// Creates a link secret for the person and returns it (the only time it is visible).
pub async fn issue(db: &DatabaseConnection, user_id: i64, purpose: &str) -> Result<String> {
    user_security_tokens::Entity::delete_many()
        .filter(user_security_tokens::Column::UserId.eq(user_id))
        .filter(user_security_tokens::Column::Purpose.eq(purpose))
        .filter(user_security_tokens::Column::UsedAt.is_null())
        .exec(db)
        .await?;
    let secret = session::new_token();
    let now = Utc::now().fixed_offset();
    user_security_tokens::ActiveModel {
        user_id: Set(user_id),
        purpose: Set(purpose.to_string()),
        token_hash: Set(session::hash_token(&secret)),
        expires_at: Set(now + Duration::minutes(lifetime_minutes(purpose))),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(secret)
}

/// Whose link this is, if it is still unused and not expired. Does not use it up.
pub async fn peek(db: &DatabaseConnection, secret: &str, purpose: &str) -> Result<Option<i64>> {
    Ok(user_security_tokens::Entity::find()
        .filter(user_security_tokens::Column::TokenHash.eq(session::hash_token(secret)))
        .filter(user_security_tokens::Column::Purpose.eq(purpose))
        .filter(user_security_tokens::Column::UsedAt.is_null())
        .filter(user_security_tokens::Column::ExpiresAt.gt(Utc::now().fixed_offset()))
        .one(db)
        .await?
        .map(|token| token.user_id))
}

/// Uses the link up and returns whose it was. Two requests with the same link cannot both win: the update only
/// matches while `used_at` is still empty.
pub async fn consume(db: &DatabaseConnection, secret: &str, purpose: &str) -> Result<Option<i64>> {
    let Some(user_id) = peek(db, secret, purpose).await? else {
        return Ok(None);
    };
    let now = Utc::now().fixed_offset();
    let result = user_security_tokens::Entity::update_many()
        .col_expr(user_security_tokens::Column::UsedAt, Expr::value(now))
        .filter(user_security_tokens::Column::TokenHash.eq(session::hash_token(secret)))
        .filter(user_security_tokens::Column::UsedAt.is_null())
        .exec(db)
        .await?;
    Ok((result.rows_affected == 1).then_some(user_id))
}

/// Removes used and expired links; returns how many.
pub async fn prune(db: &DatabaseConnection) -> Result<u64> {
    let now = Utc::now().fixed_offset();
    let result = user_security_tokens::Entity::delete_many()
        .filter(
            sea_orm::Condition::any()
                .add(user_security_tokens::Column::ExpiresAt.lt(now))
                .add(user_security_tokens::Column::UsedAt.is_not_null()),
        )
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}
