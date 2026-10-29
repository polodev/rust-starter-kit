use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        create_table(
            m,
            "contacts",
            &[
                ("id", ColType::PkAuto),
                ("name", ColType::StringLen(80)),
                ("email", ColType::StringLen(254)),
                ("subject", ColType::StringLenNull(150)),
                ("message", ColType::Text),
                ("status", ColType::StringLen(16)),
                ("replied_at", ColType::TimestampWithTimeZoneNull),
                ("locale", ColType::StringLen(8)),
                ("ip_address", ColType::StringLenNull(45)),
                ("user_agent", ColType::StringLenNull(255)),
            ],
            &[],
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "contacts").await
    }
}
