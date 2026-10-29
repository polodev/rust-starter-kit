use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        create_table(
            m,
            "media_files",
            &[
                ("id", ColType::PkAuto),
                ("model_type", ColType::StringLen(64)),
                ("model_id", ColType::BigInteger),
                ("uuid", ColType::StringLenUniq(36)),
                ("collection_name", ColType::StringLen(64)),
                ("name", ColType::StringLen(255)),
                ("file_name", ColType::StringLen(255)),
                ("mime_type", ColType::StringLen(100)),
                (
                    "disk",
                    ColType::StringLenWithDefault(32, "default".to_string()),
                ),
                ("size", ColType::BigInteger),
                ("custom_properties", ColType::Text),
                ("order_column", ColType::IntegerWithDefault(0)),
                ("is_public", ColType::BooleanWithDefault(false)),
            ],
            &[],
        )
        .await?;

        m.create_index(
            Index::create()
                .name("idx-media_files-owner-collection")
                .table(Alias::new("media_files"))
                .col(Alias::new("model_type"))
                .col(Alias::new("model_id"))
                .col(Alias::new("collection_name"))
                .to_owned(),
        )
        .await
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "media_files").await
    }
}
