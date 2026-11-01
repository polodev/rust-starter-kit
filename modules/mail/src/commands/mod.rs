use async_trait::async_trait;
use loco_rs::{
    app::AppContext,
    task::{Task, TaskInfo, Tasks, Vars},
    Error, Result,
};
use serde_json::json;

use crate::{config::MailConfig, message::Mail, queue, service};

// mods-inject (do not remove)

fn recipient(vars: &Vars) -> Result<String> {
    vars.cli_arg("to")
        .map(|address| address.trim().to_string())
        .map_err(|_| Error::string("to:<address> is required"))
}

fn show(config: &MailConfig, row: &crate::entities::mail_log::Model) {
    println!(
        "{} via {} to {} (log #{}){}",
        row.status,
        format!("{:?}", config.driver).to_lowercase(),
        row.recipients,
        row.id,
        row.provider_id
            .as_deref()
            .map(|id| format!(", provider answer: {id}"))
            .unwrap_or_default()
    );
}

/// `mail:send-test`: a short test email, to check the settings.
pub struct SendTest;

#[async_trait]
impl Task for SendTest {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "mail:send-test".to_string(),
            detail: "Send a short test email.\nUsage: cargo run -- task mail:send-test to:you@example.com".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let config = MailConfig::load(ctx)?;
        let mail = Mail::to(&recipient(vars)?)
            .subject("Test email")
            .text("This is a test email. If you can read it, sending works.")
            .html(
                "<p>This is a <strong>test email</strong>. If you can read it, sending works.</p>",
            );
        let row = service::send_with(&ctx.db, &config, mail).await?;
        show(&config, &row);
        Ok(())
    }
}

/// `mail:send-invoice`: makes the sample invoice PDF with headless Chrome and emails it.
pub struct SendInvoice;

#[async_trait]
impl Task for SendInvoice {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "mail:send-invoice".to_string(),
            detail: "Make a sample invoice PDF and email it as an attachment.\nUsage: cargo run -- task mail:send-invoice to:you@example.com".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        let config = MailConfig::load(ctx)?;
        let to = recipient(vars)?;
        let invoice = module_pdf::Invoice::sample(&to);
        println!("making the PDF for {} ...", invoice.number);
        let pdf = invoice.pdf(&module_pdf::PdfOptions::from_env()).await?;

        let data = json!({
            "seller": invoice.seller.name, "customer": invoice.customer.name, "number": invoice.number,
            "currency": invoice.currency, "total": module_pdf::invoice::money(invoice.total_minor()), "due": invoice.due,
        });
        let mail = Mail::to(&to)
            .subject(&format!(
                "Invoice {} from {}",
                invoice.number, invoice.seller.name
            ))
            .html_template(include_str!("../../templates/invoice.html"), &data)?
            .text_template(include_str!("../../templates/invoice.txt"), &data)?
            .attach_pdf(&format!("{}.pdf", invoice.number), pdf);
        println!("sending to {to} ...");
        let row = service::send_with(&ctx.db, &config, mail).await?;
        show(&config, &row);
        Ok(())
    }
}

/// `mail:ses-smtp-password`: prints the SES SMTP user name and the derived password, for Loco's own mailer.
pub struct SesSmtpPassword;

#[async_trait]
impl Task for SesSmtpPassword {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "mail:ses-smtp-password".to_string(),
            detail: "Print the Amazon SES SMTP settings (host, port, user name and the password derived from your IAM secret key) for use in Loco's own `mailer.smtp` block.\nUsage: cargo run -- task mail:ses-smtp-password".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, _vars: &Vars) -> Result<()> {
        let config = MailConfig::load(ctx)?;
        let ses = &config.ses;
        if ses.region.is_empty() || ses.access_key_id.is_empty() || ses.secret_access_key.is_empty()
        {
            return Err(Error::string(
                "set AWS_SES_DEFAULT_REGION, AWS_SES_ACCESS_KEY_ID and AWS_SES_SECRET_ACCESS_KEY in .env first",
            ));
        }
        println!("host:     {}", crate::ses::smtp_host(&ses.region));
        println!("port:     587 (STARTTLS)");
        println!("user:     {}", ses.access_key_id);
        println!(
            "password: {}",
            crate::ses::smtp_password(&ses.secret_access_key, &ses.region)
        );
        println!("\nThis password is a secret: put it in .env, never in a tracked file.");
        Ok(())
    }
}

/// Commands of this module; run them with `cargo run -- task <signature>`.
pub fn register(tasks: &mut Tasks) {
    tasks.register(SendTest);
    tasks.register(SendInvoice);
    tasks.register(SesSmtpPassword);
    tasks.register(Work);
    // tasks-inject (do not remove)
}

/// `mail:work`: sends the emails waiting in the database queue (`mail_queue`), forever, until stopped.
pub struct Work;

#[async_trait]
impl Task for Work {
    fn task(&self) -> TaskInfo {
        TaskInfo {
            name: "mail:work".to_string(),
            detail: "Send queued emails and retry failed ones. Runs until stopped (use Supervisor in production).\nUsage: cargo run -- task mail:work   (add once:true to handle what is due and exit)".to_string(),
        }
    }

    async fn run(&self, ctx: &AppContext, vars: &Vars) -> Result<()> {
        if vars.cli_arg("once").is_ok_and(|value| value == "true") {
            let config = MailConfig::load(ctx)?;
            let handled = queue::work_once(&ctx.db, &config).await?;
            println!("handled {handled} queued email(s)");
            return Ok(());
        }
        println!("mail worker running; stop it with Ctrl+C");
        queue::run_worker(ctx).await
    }
}
