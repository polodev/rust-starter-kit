use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        // Only the SHA-256 of the cookie token is stored, so a leaked table cannot be replayed.
        create_table(
            m,
            "sessions",
            &[
                ("id", ColType::PkAuto),
                ("token_hash", ColType::StringLenUniq(64)),
                ("user_agent", ColType::StringLenNull(255)),
                ("expires_at", ColType::TimestampWithTimeZone),
                ("last_used_at", ColType::TimestampWithTimeZone),
            ],
            &[("users", "")],
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "sessions").await
    }
}
