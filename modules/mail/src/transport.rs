//! Sending a built message: Amazon SES or any SMTP server, or the `log` and `capture` drivers that never
//! touch the network.

use std::{sync::Mutex, time::Duration};

use lettre::{
    transport::smtp::authentication::Credentials, AsyncSmtpTransport, AsyncTransport, Message,
    Tokio1Executor,
};
use loco_rs::{Error, Result};

use crate::{
    config::{Driver, MailConfig},
    ses,
};

/// What the provider said about an accepted message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Delivered {
    /// For SES this is the message id (the text after `Ok`), useful when asking AWS about a delivery.
    pub provider_id: Option<String>,
}

/// A message kept by the `capture` driver.
#[derive(Debug, Clone)]
pub struct Captured {
    pub recipients: Vec<String>,
    pub subject: String,
    /// The whole message as it would go over the wire.
    pub raw: Vec<u8>,
}

static CAPTURED: Mutex<Vec<Captured>> = Mutex::new(Vec::new());

/// Messages kept by the `capture` driver so far (for tests).
pub fn captured() -> Vec<Captured> {
    CAPTURED.lock().map(|list| list.clone()).unwrap_or_default()
}

pub fn clear_captured() {
    if let Ok(mut list) = CAPTURED.lock() {
        list.clear();
    }
}

fn describe(error: &lettre::transport::smtp::Error) -> String {
    error.to_string()
}

pub async fn deliver(config: &MailConfig, message: Message) -> Result<Delivered> {
    match config.driver {
        Driver::Capture => {
            let recipients = message
                .envelope()
                .to()
                .iter()
                .map(ToString::to_string)
                .collect();
            let subject = String::from_utf8_lossy(&message.formatted())
                .lines()
                .find_map(|line| line.strip_prefix("Subject: ").map(str::to_owned))
                .unwrap_or_default();
            if let Ok(mut list) = CAPTURED.lock() {
                list.push(Captured {
                    recipients,
                    subject,
                    raw: message.formatted(),
                });
            }
            Ok(Delivered::default())
        }
        Driver::Log => {
            let directory = config.log_directory();
            tokio::fs::create_dir_all(directory)
                .await
                .map_err(|error| {
                    Error::string(&format!("could not create {directory}: {error}"))
                })?;
            let path = format!(
                "{directory}/{}-{}.eml",
                chrono::Utc::now().format("%Y%m%d-%H%M%S"),
                &uuid::Uuid::new_v4().to_string()[..8]
            );
            tokio::fs::write(&path, message.formatted())
                .await
                .map_err(|error| Error::string(&error.to_string()))?;
            Ok(Delivered {
                provider_id: Some(path),
            })
        }
        Driver::Ses => {
            let ses_config = &config.ses;
            let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&ses::smtp_host(
                &ses_config.region,
            ))
            .map_err(|error| Error::string(&describe(&error)))?
            .port(587)
            .credentials(Credentials::new(
                ses_config.access_key_id.clone(),
                ses::smtp_password(&ses_config.secret_access_key, &ses_config.region),
            ))
            .timeout(Some(Duration::from_secs(30)))
            .build();
            send(transport, message).await
        }
        Driver::Smtp => {
            let smtp = &config.smtp;
            let encryption = if smtp.encryption.is_empty() {
                "starttls"
            } else {
                smtp.encryption.as_str()
            };
            let builder = match encryption {
                "none" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&smtp.host),
                "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp.host)
                    .map_err(|error| Error::string(&describe(&error)))?,
                "starttls" => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp.host)
                    .map_err(|error| Error::string(&describe(&error)))?,
                other => {
                    return Err(Error::string(&format!(
                        "smtp.encryption `{other}` is not none, starttls or tls"
                    )))
                }
            };
            let mut builder = builder.timeout(Some(Duration::from_secs(30)));
            if let Some(port) = smtp.port {
                builder = builder.port(port);
            }
            if !smtp.username.is_empty() {
                builder = builder.credentials(Credentials::new(
                    smtp.username.clone(),
                    smtp.password.clone(),
                ));
            }
            send(builder.build(), message).await
        }
    }
}

async fn send(
    transport: AsyncSmtpTransport<Tokio1Executor>,
    message: Message,
) -> Result<Delivered> {
    let response = transport
        .send(message)
        .await
        .map_err(|error| Error::string(&describe(&error)))?;
    let provider_id = response
        .message()
        .next()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty());
    Ok(Delivered { provider_id })
}
