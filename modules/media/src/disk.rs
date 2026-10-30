//! Where uploaded files are kept: the local `storage/uploads` folder, or a Cloudflare R2 (S3-compatible) bucket.
//! `MEDIA_STORAGE=r2` in `.env` switches to R2; anything else (or nothing) keeps the files on this machine.

use std::path::Path;

use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    storage::drivers::{aws, local, StoreDriver},
    task::{Task, TaskInfo, Vars},
    Error, Result,
};
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};

use crate::{
    entities::media,
    service::{disk_name, prefix},
};

/// The local folder files are kept in unless R2 is chosen.
pub const LOCAL_UPLOADS_FOLDER: &str = "storage/uploads";

/// The storage driver the app should use, from the variables in `.env`:
///
/// | Variable | Meaning |
/// |---|---|
/// | `MEDIA_STORAGE` | `r2` or `local` (default) |
/// | `R2_ENDPOINT`, `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY` | the R2 account |
/// | `R2_PUBLIC_BUCKET` | the bucket that holds the site's public files |
pub fn driver_from_environment() -> Result<Box<dyn StoreDriver>> {
    driver_from_variables(|name| std::env::var(name).ok())
}

pub fn driver_from_variables(
    variable: impl Fn(&str) -> Option<String>,
) -> Result<Box<dyn StoreDriver>> {
    let value = |name: &str| {
        variable(name)
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
    };
    let chosen = value("MEDIA_STORAGE")
        .unwrap_or_else(|| "local".to_string())
        .to_lowercase();
    let into_error = |error: loco_rs::storage::StorageError| Error::string(&error.to_string());
    match chosen.as_str() {
        "local" => local::new_with_prefix(LOCAL_UPLOADS_FOLDER).map_err(into_error),
        "r2" => {
            let need = |name: &str| {
                value(name)
                    .ok_or_else(|| Error::string(&format!("MEDIA_STORAGE=r2 needs {name} in .env")))
            };
            opendal::install_default();
            aws::with_credentials_and_endpoint(
                &need("R2_PUBLIC_BUCKET")?,
                "auto",
                &need("R2_ENDPOINT")?,
                aws::Credential {
                    key_id: need("R2_ACCESS_KEY_ID")?,
                    secret_key: need("R2_SECRET_ACCESS_KEY")?,
                    token: None,
                },
            )
            .map_err(into_error)
        }
        other => Err(Error::string(&format!(
            "MEDIA_STORAGE must be `local` or `r2`, not `{other}`"
        ))),
    }
}

/// `media:move-local-uploads`: copies files that are still in `storage/uploads` to the storage in use now (R2) and
/// points their `media` rows at the new disk. The local copies are kept; delete them yourself once you have checked.
pub struct MoveLocalUploads;

#[async_trait]
impl Task for MoveLocalUploads {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "media:move-local-uploads".to_string(),
            detail: "Copy files from storage/uploads to the current media storage (MEDIA_STORAGE=r2) and update their rows.\nUsage: cargo run -- task media:move-local-uploads [dry-run:true]".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let dry_run = vars
            .cli_arg("dry-run")
            .map(|v| v == "true")
            .unwrap_or(false);
        let target_disk = disk_name(ctx);
        let target_prefix = prefix(ctx);
        let rows = media::Entity::find().all(&ctx.db).await?;
        let (mut moved, mut skipped, mut missing) = (0, 0, 0);
        for row in rows {
            if row.disk == target_disk {
                skipped += 1;
                continue;
            }
            // Rows made before R2 was chosen were written without a prefix, in the local folder.
            let from = row.storage_path("");
            let to = row.storage_path(&target_prefix);
            let Ok(bytes) = std::fs::read(Path::new(LOCAL_UPLOADS_FOLDER).join(&from)) else {
                println!(
                    "media {} ({from}): not found in {LOCAL_UPLOADS_FOLDER}",
                    row.id
                );
                missing += 1;
                continue;
            };
            if dry_run {
                println!(
                    "media {}: would copy {from} to {to} ({} bytes)",
                    row.id,
                    bytes.len()
                );
                moved += 1;
                continue;
            }
            ctx.storage
                .upload(Path::new(&to), &bytes::Bytes::from(bytes))
                .await
                .map_err(|e| Error::string(&format!("media {}: {e}", row.id)))?;
            let id = row.id;
            let mut active = row.into_active_model();
            active.disk = Set(target_disk.clone());
            active.update(&ctx.db).await?;
            println!("media {id}: copied {from} to {to}");
            moved += 1;
        }
        println!(
            "{}{moved} copied, {skipped} already on `{target_disk}`, {missing} not found locally",
            if dry_run { "dry run: " } else { "" }
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variables(pairs: &[(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        let pairs = pairs.to_vec();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.to_string())
        }
    }

    #[test]
    fn local_is_the_default_and_r2_needs_every_variable() {
        assert!(driver_from_variables(variables(&[])).is_ok());
        assert!(driver_from_variables(variables(&[("MEDIA_STORAGE", "local")])).is_ok());
        let missing = driver_from_variables(variables(&[
            ("MEDIA_STORAGE", "r2"),
            ("R2_PUBLIC_BUCKET", "b"),
        ]))
        .err()
        .unwrap()
        .to_string();
        assert!(missing.contains("R2_ENDPOINT"), "{missing}");
        let unknown = driver_from_variables(variables(&[("MEDIA_STORAGE", "dropbox")]))
            .err()
            .unwrap()
            .to_string();
        assert!(unknown.contains("local") && unknown.contains("r2"));
        let complete = [
            ("MEDIA_STORAGE", "R2"),
            ("R2_PUBLIC_BUCKET", "bucket"),
            ("R2_ENDPOINT", "https://account.r2.cloudflarestorage.com"),
            ("R2_ACCESS_KEY_ID", "key"),
            ("R2_SECRET_ACCESS_KEY", "secret"),
        ];
        assert!(
            driver_from_variables(variables(&complete)).is_ok(),
            "no network is touched until a file is stored"
        );
    }
}
