use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        create_table(
            m,
            "pages",
            &[
                ("id", ColType::PkAuto),
                ("slug", ColType::StringLenUniq(120)),
                ("is_published", ColType::BooleanWithDefault(true)),
                ("sort_order", ColType::IntegerWithDefault(0)),
            ],
            &[],
        )
        .await?;

        create_table(
            m,
            "page_translations",
            &[
                ("id", ColType::PkAuto),
                ("locale", ColType::StringLen(8)),
                ("title", ColType::StringLen(255)),
                ("meta_description", ColType::StringLen(320)),
                ("body_html", ColType::Text),
            ],
            &[("pages", "")],
        )
        .await?;

        m.create_index(
            Index::create()
                .name("idx-page_translations-page_id-locale")
                .table(Alias::new("page_translations"))
                .col(Alias::new("page_id"))
                .col(Alias::new("locale"))
                .unique()
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "page_translations").await?;
        drop_table(m, "pages").await?;
        Ok(())
    }
}
