//! An activity log in the style of Spatie's laravel-activitylog, on the same `activity_log` table, so a
//! Laravel app and this app can write to and read from one log.
//!
//! ```ignore
//! use loco_activity_log::{activity, changes};
//!
//! activity(&ctx.db)
//!     .in_log("users")                       // inLog('users'), default log is "default"
//!     .caused_by("user", admin.id)           // causedBy($admin)
//!     .performed_on("user", target.id)       // performedOn($target)
//!     .event("updated")
//!     .with_changes(changes(&old, &new, &[])) // {"attributes": {...}, "old": {...}}, only what changed
//!     .log("Updated the profile")            // log('...')
//!     .await?;
//! ```
//!
//! Ids are stored as text (`subject_id`, `causer_id`), like a Laravel app whose models use uuid keys. Use the
//! same `subject_type` / `causer_type` strings your Laravel app stores (a morph alias or a class name); this
//! app uses short names such as `user`, `page` and `contact`.
//! Logging can be switched off with `ACTIVITY_LOGGER_ENABLED=false`.

mod entity;

pub use entity::activity_log;

use std::env;

use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
};
use serde_json::{json, Map, Value};

/// Name of the log used when none is given, like Spatie's `default_log_name`.
pub const DEFAULT_LOG: &str = "default";

/// False when `ACTIVITY_LOGGER_ENABLED` is `false`, `0` or `no`.
pub fn enabled() -> bool {
    !env::var("ACTIVITY_LOGGER_ENABLED")
        .is_ok_and(|value| matches!(value.trim().to_lowercase().as_str(), "false" | "0" | "no"))
}

/// Starts a new entry. Finish it with [`Activity::log`].
pub fn activity<C: ConnectionTrait>(db: &C) -> Activity<'_, C> {
    Activity {
        db,
        log_name: DEFAULT_LOG.to_string(),
        subject: None,
        causer: None,
        event: None,
        properties: Map::new(),
        batch_uuid: None,
    }
}

/// An activity being built, like Spatie's `ActivityLogger`.
pub struct Activity<'a, C: ConnectionTrait> {
    db: &'a C,
    log_name: String,
    subject: Option<(String, String)>,
    causer: Option<(String, String)>,
    event: Option<String>,
    properties: Map<String, Value>,
    batch_uuid: Option<String>,
}

impl<C: ConnectionTrait> Activity<'_, C> {
    /// `inLog` / `useLog`.
    pub fn in_log(mut self, name: &str) -> Self {
        self.log_name = name.to_string();
        self
    }

    /// `performedOn` / `on`: what the activity was about.
    pub fn performed_on(mut self, model_type: &str, id: impl ToString) -> Self {
        self.subject = Some((model_type.to_string(), id.to_string()));
        self
    }

    /// `causedBy` / `by`: who did it. Leave it out for an anonymous or system action.
    pub fn caused_by(mut self, model_type: &str, id: impl ToString) -> Self {
        self.causer = Some((model_type.to_string(), id.to_string()));
        self
    }

    pub fn event(mut self, event: &str) -> Self {
        self.event = Some(event.to_string());
        self
    }

    /// `withProperties`: replaces all properties. A value that is not an object is kept under `"value"`.
    pub fn with_properties(mut self, properties: Value) -> Self {
        self.properties = match properties {
            Value::Object(map) => map,
            Value::Null => Map::new(),
            other => Map::from_iter([("value".to_string(), other)]),
        };
        self
    }

    /// `withProperty`.
    pub fn with_property(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.properties.insert(key.to_string(), value.into());
        self
    }

    /// Adds the `attributes` / `old` properties produced by [`changes`], [`created`] or [`deleted`].
    pub fn with_changes(mut self, changes: Option<Value>) -> Self {
        if let Some(Value::Object(map)) = changes {
            self.properties.extend(map);
        }
        self
    }

    /// Groups related entries, like `LogBatch`. Use the same uuid for every entry of the batch.
    pub fn batch(mut self, uuid: &str) -> Self {
        self.batch_uuid = Some(uuid.to_string());
        self
    }

    /// Saves the entry. Returns `None` when logging is switched off. In the description, `:properties.key`
    /// is replaced with that property's value, as in Spatie.
    pub async fn log(self, description: &str) -> Result<Option<activity_log::Model>, DbErr> {
        if !enabled() {
            return Ok(None);
        }
        let description = replace_placeholders(description, &self.properties);
        let now = Utc::now().fixed_offset();
        let (subject_type, subject_id) = split(self.subject);
        let (causer_type, causer_id) = split(self.causer);
        let saved = activity_log::ActiveModel {
            log_name: Set(Some(self.log_name)),
            description: Set(description),
            subject_type: Set(subject_type),
            subject_id: Set(subject_id),
            event: Set(self.event),
            causer_type: Set(causer_type),
            causer_id: Set(causer_id),
            properties: Set(Some(Value::Object(self.properties).to_string())),
            batch_uuid: Set(self.batch_uuid),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
            ..Default::default()
        }
        .insert(self.db)
        .await?;
        Ok(Some(saved))
    }

    /// Like [`log`](Self::log) but never fails the caller: a problem is only written to the application log.
    /// Use it in request handlers, where a broken audit log must not break the action itself.
    pub async fn log_quietly(self, description: &str) {
        if let Err(error) = self.log(description).await {
            tracing::warn!(%error, "could not write the activity log");
        }
    }
}

fn split(pair: Option<(String, String)>) -> (Option<String>, Option<String>) {
    match pair {
        Some((kind, id)) => (Some(kind), Some(id)),
        None => (None, None),
    }
}

fn replace_placeholders(description: &str, properties: &Map<String, Value>) -> String {
    let mut out = String::with_capacity(description.len());
    let mut rest = description;
    while let Some(at) = rest.find(":properties.") {
        out.push_str(&rest[..at]);
        let after = &rest[at + ":properties.".len()..];
        let end = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(after.len());
        let key = &after[..end];
        match properties.get(key) {
            Some(Value::String(text)) => out.push_str(text),
            Some(other) if !key.is_empty() => out.push_str(&other.to_string()),
            _ => out.push_str(&rest[at..at + ":properties.".len() + end]),
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// Spatie's properties for an updated model: only the attributes that differ, new values under
/// `attributes` and previous ones under `old`. `None` when nothing changed. `ignore` names attributes to skip
/// (like `logExcept`). Pass the model as it was and as it is now, as JSON objects.
pub fn changes(old: &Value, new: &Value, ignore: &[&str]) -> Option<Value> {
    let (Some(old), Some(new)) = (old.as_object(), new.as_object()) else {
        return None;
    };
    let mut attributes = Map::new();
    let mut previous = Map::new();
    for (key, value) in new {
        if ignore.contains(&key.as_str()) {
            continue;
        }
        let before = old.get(key).unwrap_or(&Value::Null);
        if before != value {
            attributes.insert(key.clone(), value.clone());
            previous.insert(key.clone(), before.clone());
        }
    }
    (!attributes.is_empty()).then(|| json!({ "attributes": attributes, "old": previous }))
}

/// Properties for a created model: its attributes.
pub fn created(attributes: &Value) -> Option<Value> {
    attributes
        .is_object()
        .then(|| json!({ "attributes": attributes }))
}

/// Properties for a deleted model: its last attributes.
pub fn deleted(attributes: &Value) -> Option<Value> {
    created(attributes)
}

/// `activitylog:clean`: deletes entries older than `days`, optionally only from one log. Returns how many.
pub async fn clean<C: ConnectionTrait>(
    db: &C,
    days: i64,
    log_name: Option<&str>,
) -> Result<u64, DbErr> {
    let cutoff = (Utc::now() - Duration::days(days)).fixed_offset();
    let mut delete =
        activity_log::Entity::delete_many().filter(activity_log::Column::CreatedAt.lt(cutoff));
    if let Some(name) = log_name {
        delete = delete.filter(activity_log::Column::LogName.eq(name));
    }
    Ok(delete.exec(db).await?.rows_affected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_keep_only_what_differs_in_spatie_shape() {
        let old = json!({"name": "Ada", "email": "a@x.co", "updated_at": "1"});
        let new = json!({"name": "Ada L", "email": "a@x.co", "updated_at": "2"});
        assert_eq!(
            changes(&old, &new, &["updated_at"]),
            Some(json!({"attributes": {"name": "Ada L"}, "old": {"name": "Ada"}}))
        );
        assert_eq!(changes(&old, &old, &[]), None);
        assert_eq!(changes(&json!(1), &new, &[]), None);
        // A key that did not exist before shows its previous value as null.
        assert_eq!(
            changes(&json!({}), &json!({"name": "x"}), &[]),
            Some(json!({"attributes": {"name": "x"}, "old": {"name": null}}))
        );
    }

    #[test]
    fn descriptions_can_name_properties() {
        let props = json!({"name": "Ada", "n": 3}).as_object().unwrap().clone();
        assert_eq!(
            replace_placeholders(
                "Renamed to :properties.name (:properties.n times) :properties.none",
                &props
            ),
            "Renamed to Ada (3 times) :properties.none"
        );
        assert_eq!(replace_placeholders("plain", &props), "plain");
    }

    #[test]
    fn created_and_deleted_carry_the_attributes() {
        assert_eq!(
            created(&json!({"a": 1})),
            Some(json!({"attributes": {"a": 1}}))
        );
        assert_eq!(deleted(&json!(5)), None);
    }
}
