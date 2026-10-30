//! Sending and recording: every attempt is written to `mail_log` first as `queued`, then marked `sent` or
//! `failed` with the provider's answer or the error.

use chrono::Utc;
use loco_rs::{app::AppContext, Error, Result};
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};

use crate::{config::MailConfig, entities::mail_log, message::Mail, transport};

fn joined(addresses: &[&Vec<String>]) -> String {
    let all: Vec<&str> = addresses
        .iter()
        .flat_map(|list| list.iter().map(String::as_str))
        .collect();
    all.join(", ").chars().take(1000).collect()
}

/// Sends with the app's `settings.mail` and records it.
pub async fn send(ctx: &AppContext, mail: Mail) -> Result<mail_log::Model> {
    let config = MailConfig::load(ctx)?;
    send_with(&ctx.db, &config, mail).await
}

/// Sends with the given settings and records it. A delivery problem is written to the log row and returned
/// as an error; a mail that cannot even be built (no recipient, bad address) is refused before anything is logged.
pub async fn send_with<C: ConnectionTrait>(
    db: &C,
    config: &MailConfig,
    mail: Mail,
) -> Result<mail_log::Model> {
    let message = mail.build(config)?;
    let now = Utc::now().fixed_offset();
    let names: Vec<&str> = mail
        .attachments
        .iter()
        .map(|a| a.filename.as_str())
        .collect();
    let row =
        mail_log::ActiveModel {
            recipients: Set(joined(&[&mail.to, &mail.cc, &mail.bcc])),
            from_address: Set(config.from_address.clone()),
            subject: Set(mail.subject.chars().take(255).collect()),
            driver: Set(format!("{:?}", config.driver).to_lowercase()),
            status: Set(mail_log::QUEUED.to_string()),
            attempts: Set(0),
            attachments: Set(
                (!names.is_empty()).then(|| names.join(", ").chars().take(1000).collect())
            ),
            created_at: Set(Some(now)),
            updated_at: Set(Some(now)),
            ..Default::default()
        }
        .insert(db)
        .await?;

    let outcome = transport::deliver(config, message).await;
    let finished = Utc::now().fixed_offset();
    let mut active: mail_log::ActiveModel = row.into();
    active.attempts = Set(1);
    active.updated_at = Set(Some(finished));
    match outcome {
        Ok(delivered) => {
            active.status = Set(mail_log::SENT.to_string());
            active.provider_id = Set(delivered
                .provider_id
                .map(|id| id.chars().take(255).collect()));
            active.sent_at = Set(Some(finished));
            Ok(active.update(db).await?)
        }
        Err(error) => {
            let reason: String = error.to_string().chars().take(2000).collect();
            active.status = Set(mail_log::FAILED.to_string());
            active.error = Set(Some(reason.clone()));
            active.update(db).await?;
            Err(Error::string(&format!("the email was not sent: {reason}")))
        }
    }
}
