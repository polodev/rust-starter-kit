use loco_rs::testing::prelude::*;
use module_mail::{
    captured, clear_captured, enqueue, mail_log, mail_queue, send_with, work_once, Mail, MailConfig,
};
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, QueryOrder, Set};
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

fn queue_config(driver: &str) -> MailConfig {
    MailConfig::from_value(json!({
        "driver": driver, "delivery": "queue", "from_address": "support@example.com", "from_name": "Example Support",
        "smtp": { "host": "127.0.0.1", "port": 1, "encryption": "none" }
    }))
    .unwrap()
}

#[tokio::test]
#[serial]
async fn a_queued_email_waits_in_the_database_until_the_worker_sends_it() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    clear_captured();
    let config = queue_config("capture");

    let mail = Mail::to("polo@example.com")
        .subject("Queued")
        .text("Plain")
        .html("<p>Hi</p>")
        .attach_pdf("report.pdf", b"%PDF-1.4 fake".to_vec());
    let row = enqueue(&ctx.db, &config, mail).await.unwrap();
    assert_eq!(row.status, "queued");
    assert!(captured().is_empty(), "nothing is sent while queueing");
    assert_eq!(mail_queue::Entity::find().count(&ctx.db).await.unwrap(), 1);

    // A mail that cannot be built is refused before it is stored.
    assert!(
        enqueue(&ctx.db, &config, Mail::default().subject("S").text("b"))
            .await
            .is_err()
    );
    assert_eq!(mail_queue::Entity::find().count(&ctx.db).await.unwrap(), 1);

    assert_eq!(work_once(&ctx.db, &config).await.unwrap(), 1);
    let sent = rows(&ctx).await;
    assert_eq!((sent[0].status.as_str(), sent[0].attempts), ("sent", 1));
    assert!(sent[0].sent_at.is_some());
    assert_eq!(captured().len(), 1);
    assert!(String::from_utf8_lossy(&captured()[0].raw).contains("filename=\"report.pdf\""));
    assert_eq!(mail_queue::Entity::find().count(&ctx.db).await.unwrap(), 0);
    assert_eq!(
        work_once(&ctx.db, &config).await.unwrap(),
        0,
        "nothing left to do"
    );
}

#[tokio::test]
#[serial]
async fn a_failing_email_is_retried_later_and_marked_failed_after_three_attempts() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    clear_captured();
    let config = queue_config("smtp");
    enqueue(
        &ctx.db,
        &config,
        Mail::to("polo@example.com").subject("Down").text("x"),
    )
    .await
    .unwrap();

    for attempt in 1..=3 {
        let handled = work_once(&ctx.db, &config).await.unwrap();
        let log = rows(&ctx).await.remove(0);
        assert_eq!(log.attempts, attempt);
        assert!(log.error.as_deref().is_some_and(|e| !e.is_empty()));
        if attempt < 3 {
            assert_eq!(handled, 0);
            assert_eq!(log.status, "queued", "still waiting for the next try");
            let waiting = mail_queue::Entity::find()
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap();
            assert!(waiting.reserved_at.is_none() && waiting.available_at > chrono::Utc::now());
            assert_eq!(work_once(&ctx.db, &config).await.unwrap(), 0, "not due yet");
            // Make it due, as if the waiting time had passed.
            let mut due: mail_queue::ActiveModel = waiting.into();
            due.available_at =
                Set((chrono::Utc::now() - chrono::Duration::seconds(1)).fixed_offset());
            due.update(&ctx.db).await.unwrap();
        } else {
            assert_eq!(handled, 1);
            assert_eq!(log.status, "failed");
        }
    }
    assert_eq!(mail_queue::Entity::find().count(&ctx.db).await.unwrap(), 0);
}

#[tokio::test]
#[serial]
async fn two_workers_never_send_the_same_email_twice() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    clear_captured();
    let config = queue_config("capture");
    for n in 0..6 {
        enqueue(
            &ctx.db,
            &config,
            Mail::to("polo@example.com")
                .subject(&format!("Mail {n}"))
                .text("x"),
        )
        .await
        .unwrap();
    }
    let (a, b) = tokio::join!(work_once(&ctx.db, &config), work_once(&ctx.db, &config));
    assert_eq!(a.unwrap() + b.unwrap(), 6);
    assert_eq!(captured().len(), 6, "each mail was sent exactly once");
}

#[tokio::test]
#[serial]
async fn a_mail_held_by_a_crashed_worker_is_taken_over() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    clear_captured();
    let config = queue_config("capture");
    enqueue(
        &ctx.db,
        &config,
        Mail::to("polo@example.com").subject("S").text("x"),
    )
    .await
    .unwrap();
    let entry = mail_queue::Entity::find()
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap();
    let mut held: mail_queue::ActiveModel = entry.into();
    held.reserved_at = Set(Some(chrono::Utc::now().fixed_offset()));
    held.update(&ctx.db).await.unwrap();
    assert_eq!(
        work_once(&ctx.db, &config).await.unwrap(),
        0,
        "a live worker holds it"
    );

    let entry = mail_queue::Entity::find()
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap();
    let mut abandoned: mail_queue::ActiveModel = entry.into();
    abandoned.reserved_at = Set(Some(
        (chrono::Utc::now() - chrono::Duration::minutes(10)).fixed_offset(),
    ));
    abandoned.update(&ctx.db).await.unwrap();
    assert_eq!(work_once(&ctx.db, &config).await.unwrap(), 1);
    assert_eq!(captured().len(), 1);
}
