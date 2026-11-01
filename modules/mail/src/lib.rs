//! Sending email. Amazon SES (through its SMTP interface) or any SMTP server, with HTML and text bodies,
//! attachments (such as a PDF from the `pdf` module), and a `mail_log` table that records every attempt.
//! Drivers `log` (writes `.eml` files, the safe default) and `capture` (in memory, for tests) never touch the
//! network. See the README for the settings.
//!
//! ```ignore
//! let mail = Mail::to("customer@example.com")
//!     .subject("Your invoice")
//!     .html_template(include_str!("../templates/invoice.html"), &data)?
//!     .attach_pdf("invoice.pdf", pdf_bytes);
//! module_mail::send(&ctx, mail).await?;
//! ```

mod commands;
pub mod config;
mod entities;
mod message;
mod migrations;
mod queue;
mod service;
pub mod ses;
mod transport;

use async_trait::async_trait;
use loco_modular::Module;
use loco_rs::{controller::Routes, task::Tasks};
use sea_orm_migration::MigrationTrait;

pub use config::{Delivery, Driver, MailConfig};
pub use entities::{mail_log, mail_queue};
pub use message::{Attachment, Mail};
pub use queue::{dispatch, enqueue, run_worker, spawn_embedded_worker, work_once, MAX_ATTEMPTS};
pub use service::{send, send_with};
pub use transport::{captured, clear_captured, Captured};

pub struct MailModule;

#[async_trait]
impl Module for MailModule {
    fn name(&self) -> &'static str {
        "mail"
    }

    fn routes(&self) -> Vec<Routes> {
        Vec::new()
    }

    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::all()
    }

    fn register_tasks(&self, tasks: &mut Tasks) {
        commands::register(tasks);
    }
}
