//! Settings from `settings.mail` in `config/<environment>.yaml` (values come from `.env`):
//!
//! ```yaml
//! settings:
//!   mail:
//!     driver: ses            # ses | smtp | log | capture
//!     delivery: queue        # queue (default) | now (queue: stored in mail_queue, sent by `mail:work`)
//!     embedded_worker: false # true runs the queue worker inside the web server
//!     from_address: support@example.com
//!     from_name: My App
//!     ses:
//!       region: ap-southeast-1
//!       access_key_id: ...
//!       secret_access_key: ...
//!       configuration_set: ""   # optional, for SES event tracking
//!     smtp:                  # for driver: smtp (any provider, or a local Mailpit)
//!       host: localhost
//!       port: 1025
//!       username: ""
//!       password: ""
//!       encryption: none     # none | starttls | tls
//!     log_path: storage/mail # for driver: log (writes .eml files)
//! ```

use loco_rs::{app::AppContext, Error, Result};
use serde::{Deserialize, Deserializer};

fn null_as_empty<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

/// Whether `module_mail::dispatch` sends at once or puts the mail in the database queue for `mail:work`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Delivery {
    /// Send while the request waits (tests, and commands that must know the result).
    Now,
    /// Store in `mail_queue`; a worker (`mail:work`, or the embedded one in development) sends it.
    #[default]
    Queue,
}

fn bool_or_text<'de, D: Deserializer<'de>>(deserializer: D) -> std::result::Result<bool, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
        Flag(bool),
        Text(String),
        Nothing(()),
    }
    Ok(match Value::deserialize(deserializer)? {
        Value::Flag(flag) => flag,
        Value::Text(text) => matches!(text.trim().to_lowercase().as_str(), "true" | "1" | "yes"),
        Value::Nothing(()) => false,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Driver {
    /// Amazon SES through its SMTP interface.
    Ses,
    /// Any SMTP server.
    Smtp,
    /// Writes `.eml` files instead of sending: the safe default for development.
    #[default]
    Log,
    /// Keeps messages in memory, for tests.
    Capture,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct Ses {
    #[serde(default, deserialize_with = "null_as_empty")]
    pub region: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub access_key_id: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub secret_access_key: String,
    /// Optional SES configuration set, sent as `X-SES-CONFIGURATION-SET`.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub configuration_set: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Default)]
pub struct Smtp {
    #[serde(default, deserialize_with = "null_as_empty")]
    pub host: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub username: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub password: String,
    /// `none`, `starttls` (default) or `tls`.
    #[serde(default, deserialize_with = "null_as_empty")]
    pub encryption: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct MailConfig {
    #[serde(default)]
    pub driver: Driver,
    /// `queue` (default) or `now` (MAIL_DELIVERY).
    #[serde(default)]
    pub delivery: Delivery,
    /// Run the queue worker inside the web server process (handy in development; production uses `mail:work`).
    #[serde(default, deserialize_with = "bool_or_text")]
    pub embedded_worker: bool,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub from_address: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub from_name: String,
    #[serde(default)]
    pub ses: Ses,
    #[serde(default)]
    pub smtp: Smtp,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub log_path: String,
}

impl MailConfig {
    pub fn load(ctx: &AppContext) -> Result<Self> {
        let value = ctx
            .config
            .settings
            .as_ref()
            .and_then(|settings| settings.get("mail"))
            .ok_or_else(|| Error::string("mail is not configured: add settings.mail to config/<environment>.yaml (see modules/mail/README.md)"))?;
        Self::from_value(value.clone())
    }

    pub fn from_value(value: serde_json::Value) -> Result<Self> {
        let config: Self = serde_json::from_value(value)
            .map_err(|error| Error::string(&format!("settings.mail is not valid: {error}")))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if !self.from_address.contains('@') {
            return Err(Error::string(
                "settings.mail.from_address must be an email address (MAIL_FROM_ADDRESS)",
            ));
        }
        match self.driver {
            Driver::Ses => {
                let ses = &self.ses;
                if ses.region.is_empty()
                    || ses.access_key_id.is_empty()
                    || ses.secret_access_key.is_empty()
                {
                    return Err(Error::string(
                        "driver ses needs a region, access_key_id and secret_access_key (AWS_SES_DEFAULT_REGION, AWS_SES_ACCESS_KEY_ID, AWS_SES_SECRET_ACCESS_KEY)",
                    ));
                }
            }
            Driver::Smtp if self.smtp.host.is_empty() => {
                return Err(Error::string("driver smtp needs settings.mail.smtp.host"));
            }
            _ => {}
        }
        Ok(())
    }

    /// Where the `log` driver writes.
    pub fn log_directory(&self) -> &str {
        if self.log_path.is_empty() {
            "storage/mail"
        } else {
            &self.log_path
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn settings_are_read_and_checked() {
        let ses = MailConfig::from_value(json!({
            "driver": "ses", "from_address": "a@b.co", "from_name": "App",
            "ses": { "region": "ap-southeast-1", "access_key_id": "K", "secret_access_key": "S" }
        }))
        .unwrap();
        assert_eq!(ses.driver, Driver::Ses);
        assert_eq!(ses.log_directory(), "storage/mail");

        let defaults = MailConfig::from_value(json!({ "from_address": "a@b.co" })).unwrap();
        assert_eq!(
            defaults.driver,
            Driver::Log,
            "writing files is the safe default"
        );

        for (config, expected) in [
            (
                json!({ "driver": "ses", "from_address": "a@b.co", "ses": { "region": "r", "access_key_id": null } }),
                "driver ses needs",
            ),
            (
                json!({ "driver": "smtp", "from_address": "a@b.co" }),
                "smtp.host",
            ),
            (
                json!({ "driver": "log", "from_address": "nope" }),
                "from_address",
            ),
            (
                json!({ "driver": "carrier-pigeon", "from_address": "a@b.co" }),
                "not valid",
            ),
        ] {
            let error = MailConfig::from_value(config).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }
}
