//! Settings for the database backup, read from `settings.db_backup` in `config/<environment>.yaml`.
//! They are separate from the website's own storage, so backups can go to a different bucket and account.
//!
//! ```yaml
//! settings:
//!   db_backup:
//!     name: my-app                       # folder inside every disk
//!     destinations: [local, r2]          # where `backup:run` stores the dump
//!     disks:
//!       local:
//!         driver: local
//!         path: storage/backups
//!       r2:
//!         driver: s3                     # AWS S3, Cloudflare R2, MinIO, Backblaze B2 ...
//!         bucket: my-backups
//!         region: auto
//!         endpoint: https://<account>.r2.cloudflarestorage.com
//!         access_key_id: <%= get_env(name="BACKUP_R2_KEY", default="") %>
//!         secret_access_key: <%= get_env(name="BACKUP_R2_SECRET", default="") %>
//!         prefix: db                     # optional folder inside the bucket
//!     retention:                         # Spatie's default strategy
//!       keep_all_days: 7
//!       keep_daily_days: 16
//!       keep_weekly_weeks: 8
//!       keep_monthly_months: 4
//!       keep_yearly_years: 2
//!     monitor_max_age_days: 1
//! ```

use std::collections::HashMap;

use loco_rs::{app::AppContext, Error, Result};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BackupConfig {
    /// `false` switches backups off (the settings are then not even checked). Handy with an environment
    /// variable: `enabled: <%= get_env(name="DB_BACKUP_ENABLED", default="false") %>`.
    #[serde(default = "default_enabled", deserialize_with = "flexible_bool")]
    pub enabled: bool,
    /// Folder that holds this app's backups on every disk (like Spatie's `backup.name`).
    #[serde(default = "default_name", deserialize_with = "name_or_default")]
    pub name: String,
    /// Disk names that `backup:run` writes to.
    #[serde(default)]
    pub destinations: Vec<String>,
    #[serde(default)]
    pub disks: HashMap<String, DiskConfig>,
    #[serde(default)]
    pub retention: Retention,
    /// `backup:monitor` complains when the newest backup is older than this many days.
    #[serde(default = "default_monitor_days")]
    pub monitor_max_age_days: i64,
    /// Database to back up, `mysql://...` or `postgres://...`. Empty means the app's own `database.uri`.
    #[serde(default, deserialize_with = "optional_text")]
    pub database_uri: Option<String>,
    /// Path of `mysqldump`, `mariadb-dump` or `pg_dump` when it is not on the `PATH`.
    #[serde(default)]
    pub dump_binary: Option<String>,
    /// Extra arguments for the dump command, for example `["--no-tablespaces"]`.
    #[serde(default)]
    pub extra_dump_args: Vec<String>,
}

/// Empty text and `null` (an unset environment variable) both mean "not set".
fn optional_text<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty()))
}

/// An empty environment variable becomes `null` in the YAML; read that as an empty text.
fn null_as_empty<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

fn name_or_default<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(default_name))
}

/// `true`, `"true"`, `"1"` and `"yes"` mean on; anything else means off.
fn flexible_bool<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<bool, D::Error> {
    Ok(match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::Bool(flag) => flag,
        serde_json::Value::String(text) => {
            matches!(text.trim().to_lowercase().as_str(), "true" | "1" | "yes")
        }
        _ => false,
    })
}

fn default_enabled() -> bool {
    true
}

fn default_name() -> String {
    "backups".to_string()
}

fn default_monitor_days() -> i64 {
    1
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(tag = "driver", rename_all = "lowercase")]
pub enum DiskConfig {
    /// A folder on this machine.
    Local {
        #[serde(default, deserialize_with = "null_as_empty")]
        path: String,
    },
    /// Any S3-compatible bucket.
    S3 {
        #[serde(default, deserialize_with = "null_as_empty")]
        bucket: String,
        #[serde(default = "default_region")]
        region: String,
        /// Needed for R2, MinIO, B2 and other non-AWS services.
        #[serde(default)]
        endpoint: Option<String>,
        #[serde(default, deserialize_with = "null_as_empty")]
        access_key_id: String,
        #[serde(default, deserialize_with = "null_as_empty")]
        secret_access_key: String,
        /// Folder inside the bucket.
        #[serde(default)]
        prefix: Option<String>,
    },
}

fn default_region() -> String {
    "auto".to_string()
}

/// Spatie's `DefaultStrategy`: keep everything for a while, then one backup per day, week, month and year.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Retention {
    pub keep_all_days: i64,
    pub keep_daily_days: i64,
    pub keep_weekly_weeks: i64,
    pub keep_monthly_months: i64,
    pub keep_yearly_years: i64,
}

impl Default for Retention {
    fn default() -> Self {
        Self {
            keep_all_days: 7,
            keep_daily_days: 16,
            keep_weekly_weeks: 8,
            keep_monthly_months: 4,
            keep_yearly_years: 2,
        }
    }
}

impl BackupConfig {
    /// Reads `settings.db_backup`; `Ok(None)` when it is missing or switched off with `enabled: false`.
    pub fn load(ctx: &AppContext) -> Result<Option<Self>> {
        match ctx
            .config
            .settings
            .as_ref()
            .and_then(|settings| settings.get("db_backup"))
        {
            Some(value) => Self::from_settings(value.clone()),
            None => Ok(None),
        }
    }

    /// Like [`load`](Self::load) for a settings value. A switched-off block is not read any further, so an
    /// empty bucket or key (no environment variables yet) is fine until it is turned on.
    pub fn from_settings(value: serde_json::Value) -> Result<Option<Self>> {
        let enabled = match value.get("enabled") {
            None => true,
            Some(serde_json::Value::Bool(flag)) => *flag,
            Some(serde_json::Value::String(text)) => {
                matches!(text.trim().to_lowercase().as_str(), "true" | "1" | "yes")
            }
            Some(_) => false,
        };
        if !enabled {
            return Ok(None);
        }
        Self::from_value(value).map(Some)
    }

    pub fn from_value(value: serde_json::Value) -> Result<Self> {
        let config: Self = serde_json::from_value(value)
            .map_err(|error| Error::string(&format!("settings.db_backup is not valid: {error}")))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.destinations.is_empty() {
            return Err(Error::string(
                "settings.db_backup.destinations is empty: name at least one disk",
            ));
        }
        for name in &self.destinations {
            if !self.disks.contains_key(name) {
                return Err(Error::string(&format!(
                    "settings.db_backup.destinations names `{name}`, but no disk has that name"
                )));
            }
        }
        for (name, disk) in &self.disks {
            match disk {
                DiskConfig::Local { path } if path.trim().is_empty() => {
                    return Err(Error::string(&format!("disk `{name}` needs a path")));
                }
                DiskConfig::S3 {
                    bucket,
                    access_key_id,
                    secret_access_key,
                    ..
                } if bucket.trim().is_empty()
                    || access_key_id.trim().is_empty()
                    || secret_access_key.trim().is_empty() =>
                {
                    return Err(Error::string(&format!(
                        "disk `{name}` needs a bucket, access_key_id and secret_access_key (check the environment variables)"
                    )));
                }
                _ => {}
            }
        }
        let valid_name = !self.name.is_empty()
            && self
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
        if !valid_name {
            return Err(Error::string(
                "settings.db_backup.name may only use letters, digits, - _ and .",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn valid() -> serde_json::Value {
        json!({
            "name": "my-app",
            "destinations": ["local", "r2"],
            "disks": {
                "local": { "driver": "local", "path": "storage/backups" },
                "r2": { "driver": "s3", "bucket": "b", "endpoint": "https://x.r2.cloudflarestorage.com",
                        "access_key_id": "k", "secret_access_key": "s", "prefix": "db" }
            },
            "retention": { "keep_all_days": 3 }
        })
    }

    #[test]
    fn a_full_configuration_is_read_with_defaults_filled_in() {
        let config = BackupConfig::from_value(valid()).unwrap();
        assert_eq!(config.name, "my-app");
        assert_eq!(config.retention.keep_all_days, 3);
        assert_eq!(
            config.retention.keep_daily_days, 16,
            "unset values use Spatie's defaults"
        );
        assert_eq!(config.monitor_max_age_days, 1);
        assert!(matches!(&config.disks["r2"], DiskConfig::S3 { region, .. } if region == "auto"));
    }

    #[test]
    fn a_switched_off_block_is_not_checked() {
        // No environment variables yet: empty keys and nulls are fine while it is off.
        let off = json!({
            "enabled": false, "destinations": ["offsite"],
            "disks": { "offsite": { "driver": "s3", "bucket": null, "access_key_id": null, "secret_access_key": null } }
        });
        assert_eq!(BackupConfig::from_settings(off).unwrap(), None);
        let off_as_text = json!({ "enabled": "false", "destinations": [] });
        assert_eq!(BackupConfig::from_settings(off_as_text).unwrap(), None);
        // Turned on with the keys missing, the problem is explained.
        let on = json!({
            "enabled": "true", "destinations": ["offsite"],
            "disks": { "offsite": { "driver": "s3", "bucket": null, "access_key_id": null, "secret_access_key": null } }
        });
        assert!(BackupConfig::from_settings(on)
            .unwrap_err()
            .to_string()
            .contains("needs a bucket"));
        assert!(BackupConfig::from_settings(valid()).unwrap().is_some());
    }

    #[test]
    fn mistakes_are_explained() {
        let mut no_destination = valid();
        no_destination["destinations"] = json!([]);
        let mut unknown_disk = valid();
        unknown_disk["destinations"] = json!(["nowhere"]);
        let mut empty_key = valid();
        empty_key["disks"]["r2"]["secret_access_key"] = json!("");
        let mut bad_name = valid();
        bad_name["name"] = json!("../escape");
        for (config, expected) in [
            (no_destination, "destinations is empty"),
            (unknown_disk, "no disk has that name"),
            (empty_key, "needs a bucket"),
            (bad_name, "may only use"),
        ] {
            let error = BackupConfig::from_value(config).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
        assert!(BackupConfig::from_value(
            json!({"destinations": ["a"], "disks": {"a": {"driver": "ftp"}}})
        )
        .is_err());
    }
}
