use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Only the SHA-256 of the secret half of the token is stored, like the web sessions.
        create_table(
            m,
            "personal_access_tokens",
            &[
                ("id", ColType::PkAuto),
                ("name", ColType::String),
                ("token_hash", ColType::StringLenUniq(64)),
                ("last_used_at", ColType::TimestampWithTimeZoneNull),
                ("expires_at", ColType::TimestampWithTimeZoneNull),
            ],
            &[("users", "")],
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "personal_access_tokens").await
    }
}
