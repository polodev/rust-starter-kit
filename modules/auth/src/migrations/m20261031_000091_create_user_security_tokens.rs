use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // One-time links sent by email (verify the address, reset the password, sign in). Like sessions, only the
        // SHA-256 of the secret in the link is stored.
        create_table(
            m,
            "user_security_tokens",
            &[
                ("id", ColType::PkAuto),
                ("purpose", ColType::StringLen(40)),
                ("token_hash", ColType::StringLenUniq(64)),
                ("expires_at", ColType::TimestampWithTimeZone),
                ("used_at", ColType::TimestampWithTimeZoneNull),
            ],
            &[("users", "")],
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "user_security_tokens").await
    }
}
