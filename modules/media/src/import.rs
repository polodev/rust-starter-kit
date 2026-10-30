//! `media:import <database>`: copies the rows of another database's Spatie `media` table into this
//! one, keeping their ids, so the files that already sit at `{id}/{file_name}` in the shared bucket
//! keep working. Run it again later to pick up newer rows; rows already present are left alone.

use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Vars},
    Error, Result,
};
use sea_orm::ConnectionTrait;

pub struct ImportMedia;

fn valid_database_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

#[async_trait]
impl Task for ImportMedia {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "media:import".to_string(),
            detail: "Copy a Laravel (Spatie) media table from another database on the same server.\nUsage: cargo run -- task media:import database:bimafy_sep24".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let source = vars
            .cli
            .get("database")
            .ok_or_else(|| Error::string("pass the source database: database:<name>"))?;
        if !valid_database_name(source) {
            return Err(Error::string(
                "the database name may only use letters, digits and _",
            ));
        }
        let columns = "id, model_type, model_id, uuid, collection_name, name, file_name, mime_type, disk, \
                       conversions_disk, size, manipulations, custom_properties, generated_conversions, \
                       responsive_images, order_column, created_at, updated_at";
        let sql =
            format!("INSERT IGNORE INTO media ({columns}) SELECT {columns} FROM `{source}`.media");
        let result = ctx.db.execute_unprepared(&sql).await?;
        println!(
            "imported {} media row(s) from {source}",
            result.rows_affected()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::valid_database_name;

    #[test]
    fn only_plain_database_names_are_accepted() {
        assert!(valid_database_name("bimafy_sep24"));
        for bad in ["", "a b", "a`b", "x;drop", "a.b", &"x".repeat(65)] {
            assert!(!valid_database_name(bad), "{bad}");
        }
    }
}
