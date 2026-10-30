use loco_rs::testing::prelude::*;
use module_mail::{captured, clear_captured, mail_log, send_with, Mail, MailConfig};
use sea_orm::{EntityTrait, PaginatorTrait, QueryOrder};
use serde_json::json;
use serial_test::serial;

use rust_starter::app::App;

fn config(driver: &str) -> MailConfig {
    MailConfig::from_value(json!({
        "driver": driver, "from_address": "support@example.com", "from_name": "Example Support",
        "smtp": { "host": "127.0.0.1", "port": 1, "encryption": "none" }
    }))
    .unwrap()
}

async fn rows(ctx: &loco_rs::app::AppContext) -> Vec<mail_log::Model> {
    mail_log::Entity::find()
        .order_by_asc(mail_log::Column::Id)
        .all(&ctx.db)
        .await
        .unwrap()
}

#[tokio::test]
#[serial]
async fn a_sent_email_is_captured_and_logged_and_a_failed_one_is_logged_with_its_error() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    clear_captured();

    let mail = Mail::to("polo@example.com")
        .cc("boss@example.com")
        .subject("Hello")
        .text("Plain body")
        .html("<p>HTML body</p>")
        .attach_pdf("report.pdf", b"%PDF-1.4 fake".to_vec());
    let sent = send_with(&ctx.db, &config("capture"), mail).await.unwrap();
    assert_eq!(
        (sent.status.as_str(), sent.attempts, sent.driver.as_str()),
        ("sent", 1, "capture")
    );
    assert_eq!(sent.recipients, "polo@example.com, boss@example.com");
    assert_eq!(sent.attachments.as_deref(), Some("report.pdf"));
    assert!(sent.sent_at.is_some() && sent.error.is_none());

    let messages = captured();
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].recipients,
        vec!["polo@example.com", "boss@example.com"]
    );
    assert_eq!(messages[0].subject, "Hello");
    let raw = String::from_utf8_lossy(&messages[0].raw);
    assert!(
        // A tiny ASCII body is sent as is; real (binary) PDFs are base64-encoded. Either way it is attached.
        raw.contains("filename=\"report.pdf\"")
            && (raw.contains("JVBERi0") || raw.contains("%PDF-1.4 fake")),
        "the PDF is attached"
    );

    // A server that cannot be reached: the attempt is recorded as failed and the error comes back.
    let error = send_with(
        &ctx.db,
        &config("smtp"),
        Mail::to("polo@example.com").subject("Down").text("x"),
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(error.contains("not sent"), "{error}");
    let all = rows(&ctx).await;
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].status, "failed");
    assert!(all[1].error.as_deref().is_some_and(|e| !e.is_empty()) && all[1].sent_at.is_none());

    // A mail that cannot be built is refused before anything is logged or sent.
    assert!(send_with(
        &ctx.db,
        &config("capture"),
        Mail::to("not-an-email").subject("S").text("b")
    )
    .await
    .is_err());
    assert!(send_with(
        &ctx.db,
        &config("capture"),
        Mail::default().subject("S").text("b")
    )
    .await
    .is_err());
    assert_eq!(mail_log::Entity::find().count(&ctx.db).await.unwrap(), 2);
    assert_eq!(captured().len(), 1);
}

#[tokio::test]
#[serial]
async fn the_invoice_pdf_is_made_with_chrome_and_sent_as_an_attachment() {
    if module_pdf::find_chrome(None).is_err() {
        eprintln!("skipped: Chrome is not installed");
        return;
    }
    let ctx = boot_test::<App>().await.unwrap().app_context;
    clear_captured();
    let invoice = module_pdf::Invoice::sample("customer@example.com");
    let pdf = invoice
        .pdf(&module_pdf::PdfOptions::from_env())
        .await
        .unwrap();
    assert!(
        pdf.starts_with(b"%PDF-") && pdf.len() > 20_000,
        "a real, non-trivial PDF ({} bytes)",
        pdf.len()
    );

    let data = json!({
        "seller": invoice.seller.name, "customer": invoice.customer.name, "number": invoice.number,
        "currency": invoice.currency, "total": module_pdf::invoice::money(invoice.total_minor()), "due": invoice.due,
    });
    let mail = Mail::to("customer@example.com")
        .subject(&format!("Invoice {}", invoice.number))
        .html_template(
            include_str!("../../modules/mail/templates/invoice.html"),
            &data,
        )
        .unwrap()
        .text_template(
            include_str!("../../modules/mail/templates/invoice.txt"),
            &data,
        )
        .unwrap()
        .attach_pdf(&format!("{}.pdf", invoice.number), pdf);
    let row = send_with(&ctx.db, &config("capture"), mail).await.unwrap();
    assert_eq!(row.status, "sent");
    let raw = String::from_utf8_lossy(&captured()[0].raw).into_owned();
    assert!(
        raw.contains("61,525.00") && raw.contains("Amount due"),
        "the amount is in the body"
    );
    assert!(raw.contains(&format!("{}.pdf", invoice.number)));
}
