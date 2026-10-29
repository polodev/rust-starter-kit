use loco_rs::schema::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        create_table(
            m,
            "roles",
            &[
                ("id", ColType::PkAuto),
                ("slug", ColType::StringLenUniq(64)),
                ("name", ColType::StringLen(120)),
                ("rank", ColType::Integer),
                ("description", ColType::TextNull),
                ("is_active", ColType::BooleanWithDefault(true)),
            ],
            &[],
        )
        .await?;

        create_table(
            m,
            "permissions",
            &[
                ("id", ColType::PkAuto),
                ("key", ColType::StringLenUniq(120)),
                ("name", ColType::StringLen(255)),
                ("group_name", ColType::StringLen(64)),
                ("is_active", ColType::BooleanWithDefault(true)),
            ],
            &[],
        )
        .await?;

        create_join_table(
            m,
            "permission_roles",
            &[],
            &[("roles", ""), ("permissions", "")],
        )
        .await?;
        create_join_table(m, "role_users", &[], &[("users", ""), ("roles", "")]).await?;
        create_join_table(
            m,
            "permission_users",
            &[],
            &[("users", ""), ("permissions", "")],
        )
        .await?;

        // One role per user, like Dokani; direct permission exceptions live in permission_user.
        m.create_index(
            Index::create()
                .name("idx-role_users-user_id-unique")
                .table(Alias::new("role_users"))
                .col(Alias::new("user_id"))
                .unique()
                .to_owned(),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        drop_table(m, "permission_users").await?;
        drop_table(m, "role_users").await?;
        drop_table(m, "permission_roles").await?;
        drop_table(m, "permissions").await?;
        drop_table(m, "roles").await?;
        Ok(())
    }
}
