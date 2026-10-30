//! Building an email: recipients, subject, HTML and text bodies, attachments. The result is a `lettre` message.

use lettre::{
    message::{
        header::ContentType, Attachment as LettreAttachment, Mailbox, MultiPart, SinglePart,
    },
    Address, Message,
};
use loco_rs::{Error, Result};
use serde_json::Value;

use crate::config::MailConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// An email, built step by step like a Laravel `Mailable`:
///
/// ```ignore
/// Mail::to("customer@example.com")
///     .subject("Your invoice")
///     .html_template(include_str!("../templates/invoice.html"), &data)?
///     .text("See the attached PDF.")
///     .attach_pdf("invoice.pdf", pdf_bytes)
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mail {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub reply_to: Option<String>,
    pub subject: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub attachments: Vec<Attachment>,
}

impl Mail {
    pub fn to(address: &str) -> Self {
        Self {
            to: vec![address.trim().to_string()],
            ..Self::default()
        }
    }

    pub fn also_to(mut self, address: &str) -> Self {
        self.to.push(address.trim().to_string());
        self
    }

    pub fn cc(mut self, address: &str) -> Self {
        self.cc.push(address.trim().to_string());
        self
    }

    pub fn bcc(mut self, address: &str) -> Self {
        self.bcc.push(address.trim().to_string());
        self
    }

    pub fn reply_to(mut self, address: &str) -> Self {
        self.reply_to = Some(address.trim().to_string());
        self
    }

    pub fn subject(mut self, subject: &str) -> Self {
        self.subject = subject.trim().to_string();
        self
    }

    pub fn text(mut self, text: &str) -> Self {
        self.text = Some(text.to_string());
        self
    }

    pub fn html(mut self, html: &str) -> Self {
        self.html = Some(html.to_string());
        self
    }

    /// Renders a Tera template into the HTML body. Values are HTML-escaped.
    pub fn html_template(self, source: &str, data: &Value) -> Result<Self> {
        let html = module_pdf::render_template(source, data)?;
        Ok(self.html(&html))
    }

    /// Renders a Tera template into the plain text body.
    pub fn text_template(self, source: &str, data: &Value) -> Result<Self> {
        let context = tera::Context::from_value(data.clone())
            .map_err(|error| Error::string(&format!("bad template data: {error}")))?;
        // Plain text: no HTML escaping.
        let text = tera::Tera::one_off(source, &context, false)
            .map_err(|error| Error::string(&format!("could not render the template: {error:?}")))?;
        Ok(self.text(&text))
    }

    pub fn attach(mut self, filename: &str, content_type: &str, bytes: Vec<u8>) -> Self {
        self.attachments.push(Attachment {
            filename: filename.to_string(),
            content_type: content_type.to_string(),
            bytes,
        });
        self
    }

    pub fn attach_pdf(self, filename: &str, bytes: Vec<u8>) -> Self {
        self.attach(filename, "application/pdf", bytes)
    }

    /// Plain text made from the HTML, for mail programs that do not show HTML.
    fn text_from_html(html: &str) -> String {
        let mut out = String::new();
        let mut in_tag = false;
        for c in html.chars() {
            match c {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    out.push(' ');
                }
                c if !in_tag => out.push(c),
                _ => {}
            }
        }
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn mailbox(address: &str) -> Result<Mailbox> {
        let parsed: Address = address
            .parse()
            .map_err(|_| Error::string(&format!("`{address}` is not a valid email address")))?;
        Ok(Mailbox::new(None, parsed))
    }

    /// Checks the mail and turns it into a message ready to send.
    pub fn build(&self, config: &MailConfig) -> Result<Message> {
        if self.to.is_empty() {
            return Err(Error::string("the email has no recipient"));
        }
        if self.subject.is_empty() || self.subject.contains(['\r', '\n']) {
            return Err(Error::string("the subject must be one line of text"));
        }
        let html = self.html.as_deref().filter(|html| !html.trim().is_empty());
        let text = self.text.as_deref().filter(|text| !text.trim().is_empty());
        if html.is_none() && text.is_none() {
            return Err(Error::string("the email has no body"));
        }

        let from_address: Address = config.from_address.parse().map_err(|_| {
            Error::string("settings.mail.from_address is not a valid email address")
        })?;
        let from_name = (!config.from_name.is_empty()).then(|| config.from_name.clone());
        let domain = from_address.domain().to_string();
        let mut builder = Message::builder()
            .from(Mailbox::new(from_name, from_address))
            .subject(self.subject.clone())
            .message_id(Some(format!("<{}@{domain}>", uuid::Uuid::new_v4())));
        for address in &self.to {
            builder = builder.to(Self::mailbox(address)?);
        }
        for address in &self.cc {
            builder = builder.cc(Self::mailbox(address)?);
        }
        for address in &self.bcc {
            builder = builder.bcc(Self::mailbox(address)?);
        }
        if let Some(address) = &self.reply_to {
            builder = builder.reply_to(Self::mailbox(address)?);
        }
        if !config.ses.configuration_set.is_empty() {
            use lettre::message::header::{HeaderName, HeaderValue};
            builder = builder.raw_header(HeaderValue::new(
                HeaderName::new_from_ascii_str("X-SES-CONFIGURATION-SET"),
                config.ses.configuration_set.clone(),
            ));
        }

        let body = match (text, html) {
            (Some(text), Some(html)) => {
                MultiPart::alternative_plain_html(text.to_string(), html.to_string())
            }
            (None, Some(html)) => {
                MultiPart::alternative_plain_html(Self::text_from_html(html), html.to_string())
            }
            (Some(text), None) => {
                MultiPart::alternative().singlepart(SinglePart::plain(text.to_string()))
            }
            (None, None) => unreachable!("checked above"),
        };
        let message = if self.attachments.is_empty() {
            builder.multipart(body)
        } else {
            let mut mixed = MultiPart::mixed().multipart(body);
            for attachment in &self.attachments {
                let content_type = ContentType::parse(&attachment.content_type).map_err(|_| {
                    Error::string(&format!(
                        "`{}` is not a content type",
                        attachment.content_type
                    ))
                })?;
                mixed = mixed.singlepart(
                    LettreAttachment::new(attachment.filename.clone())
                        .body(attachment.bytes.clone(), content_type),
                );
            }
            builder.multipart(mixed)
        };
        message.map_err(|error| Error::string(&format!("could not build the email: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::config::{Driver, MailConfig};

    fn config() -> MailConfig {
        MailConfig::from_value(json!({ "driver": "capture", "from_address": "support@example.com", "from_name": "Example Support" })).unwrap()
    }

    fn eml(mail: &Mail) -> String {
        String::from_utf8_lossy(&mail.build(&config()).unwrap().formatted()).into_owned()
    }

    #[test]
    fn a_message_has_headers_both_bodies_and_the_attachment() {
        let mail = Mail::to("polo@example.com")
            .cc("boss@example.com")
            .subject("Invoice INV-1")
            .html("<p>Hello <b>there</b></p>")
            .attach_pdf("invoice.pdf", b"%PDF-1.4 fake".to_vec());
        let text = eml(&mail);
        for needle in [
            "From: \"Example Support\" <support@example.com>",
            "To: polo@example.com",
            "Cc: boss@example.com",
            "Subject: Invoice INV-1",
            "multipart/mixed",
            "multipart/alternative",
            "text/plain",
            "text/html",
            "application/pdf",
            "invoice.pdf",
            "Message-ID: <",
        ] {
            assert!(text.contains(needle), "missing {needle}\n{text}");
        }
        assert!(
            !text.contains("Bcc:"),
            "blind copies are not written into the message"
        );
    }

    #[test]
    fn the_text_part_is_made_from_the_html_when_missing() {
        assert_eq!(
            Mail::text_from_html("<h1>Hi</h1><p>Total <b>BDT 5</b></p>"),
            "Hi Total BDT 5"
        );
    }

    #[test]
    fn bad_mail_is_refused_before_anything_is_sent() {
        let ok = Mail::to("a@b.co").subject("S").text("body");
        assert!(ok.build(&config()).is_ok());
        for (mail, expected) in [
            (Mail::default().subject("S").text("b"), "no recipient"),
            (
                Mail::to("not-an-email").subject("S").text("b"),
                "not a valid email",
            ),
            (Mail::to("a@b.co").text("b"), "subject"),
            (
                Mail::to("a@b.co").subject("two\nlines").text("b"),
                "subject",
            ),
            (Mail::to("a@b.co").subject("S"), "no body"),
            (
                Mail::to("a@b.co")
                    .subject("S")
                    .text("b")
                    .attach("x", "not a type", vec![1]),
                "content type",
            ),
        ] {
            let error = mail.build(&config()).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
        let _ = Driver::Capture;
    }

    #[test]
    fn templates_render_into_the_bodies_and_escape_html() {
        let mail = Mail::to("a@b.co")
            .subject("S")
            .html_template("<p>Hi {{ name }}</p>", &json!({ "name": "<script>" }))
            .unwrap()
            .text_template("Hi {{ name }}", &json!({ "name": "Ada & Bob" }))
            .unwrap();
        assert_eq!(mail.html.as_deref(), Some("<p>Hi &lt;script&gt;</p>"));
        assert_eq!(mail.text.as_deref(), Some("Hi Ada & Bob"));
    }

    #[test]
    fn the_ses_configuration_set_header_is_added_when_set() {
        let mut config = config();
        config.ses.configuration_set = "tracking".into();
        let text = String::from_utf8_lossy(
            &Mail::to("a@b.co")
                .subject("S")
                .text("b")
                .build(&config)
                .unwrap()
                .formatted(),
        )
        .into_owned();
        assert!(text.contains("X-SES-CONFIGURATION-SET: tracking"));
    }
}
