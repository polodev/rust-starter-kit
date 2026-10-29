use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        create_table(
            m,
            "social_accounts",
            &[
                ("id", ColType::PkAuto),
                ("provider", ColType::String),
                ("identity", ColType::StringLenUniq(255)),
                ("email", ColType::String),
            ],
            &[("users", "")],
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "social_accounts").await
    }
}
