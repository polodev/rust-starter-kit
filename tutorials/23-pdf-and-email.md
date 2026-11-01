<!-- nav:start -->
**Tutorial 23 of 28** | [Index](00-index.md) | [Previous: 22 Activity log, queue monitor and database backups](22-activity-log-queue-and-backups.md) | [Next: 24 Email and PDF step by step](24-email-and-pdf-step-by-step.md)

<details><summary>Jump to any tutorial</summary>

[00. Index](00-index.md)  
01. [Introduction](01-introduction.md)  
02. [Setup](02-setup.md)  
03. [Rust essentials](03-rust-essentials.md)  
04. [Project structure](04-project-structure.md)  
05. [Modules and `cargo modular`](05-modules.md)  
06. [Routes, controllers and views](06-routes-controllers-views.md)  
07. [Database: migrations, entities, queries, seeding](07-database.md)  
08. [A bilingual site (English and Bangla)](08-bilingual-site.md)  
09. [Cache and sessions](09-cache-and-sessions.md)  
10. [Testing](10-testing.md)  
11. [Assets and styling](11-assets-and-styling.md)  
12. [Cheat sheet and what to build next](12-cheatsheet.md)  
13. [Console commands and the scheduler](13-console-and-schedule.md)  
14. [Login, register and sessions (`auth` module)](14-auth-and-sessions.md)  
15. [Roles and permissions (`role-permission` module)](15-roles-and-permissions.md)  
16. [The admin dashboard (`admin-layout` module)](16-admin-dashboard.md)  
17. [Media library (`media` module)](17-media-library.md)  
18. [JSON API and Google login](18-json-api-and-google-login.md)  
19. [Components and the contact form](19-components-and-contact-form.md)  
20. [Tinker and sharing the database with Laravel](20-tinker-and-laravel-database.md)  
21. [Conventions](21-conventions.md)  
22. [Activity log, queue monitor and database backups](22-activity-log-queue-and-backups.md)  
**23. [PDF and email](23-pdf-and-email.md)**  
24. [Email and PDF step by step](24-email-and-pdf-step-by-step.md)  
25. [AI service: Claude, OpenAI and Gemini](25-ai-service.md)  
26. [Commands and jobs for Laravel developers](26-commands-and-jobs-for-laravel-developers.md)  
27. [PostgreSQL for MySQL users](27-postgresql-for-mysql-users.md)  
28. [MySQL or PostgreSQL, one choice per app](28-mysql-or-postgresql-per-app.md)  

</details>
<!-- nav:end -->

# 23. PDF and email (`pdf`, `mail`)

## PDF: headless Chrome, like Browsershot

`pdf` renders HTML and CSS in headless Chrome and prints it. Bangla text is shaped by the browser, so it looks right.

```rust
let html = module_pdf::render_template(include_str!("../templates/pdf/invoice.html"), &data)?;   // Tera
let pdf = module_pdf::html_to_pdf(&html, &PdfOptions::from_env()).await?;                       // Vec<u8>
```

- **Needs** Chrome or Chromium installed (set `CHROME_PATH` if it is not found; `PDF_CHROME_NO_SANDBOX=true` as root in a container). Install a Bangla font on the server.
- Page size and margins are CSS: `@page { size: A4; margin: 18mm }`.
- Try it: `cargo run -- task pdf:sample out:/tmp/invoice.pdf`. The sample is an invoice with exact money maths and Bangla lines.

## Email: Amazon SES, or any SMTP

```rust
let mail = Mail::to("customer@example.com")
    .subject("Your invoice")
    .html_template(include_str!("../templates/invoice.html"), &data)?
    .text_template(include_str!("../templates/invoice.txt"), &data)?
    .attach_pdf("invoice.pdf", pdf);
module_mail::send(&ctx, mail).await?;
```

Every send is written to `mail_log` first as `queued`, then marked `sent` (with the provider's answer) or `failed` (with the error). A mail that cannot be built is refused before anything happens.

Turn it on in `.env`:

```sh
MAIL_DRIVER=ses                  # log (default: writes .eml files, sends nothing) | ses | smtp
MAIL_FROM_ADDRESS=support@example.com
MAIL_FROM_NAME=My App
AWS_SES_ACCESS_KEY_ID=...
AWS_SES_SECRET_ACCESS_KEY=...
AWS_SES_DEFAULT_REGION=ap-southeast-1
```

- **SES over SMTP:** the module connects to `email-smtp.<region>.amazonaws.com:587` and derives the SMTP password from the IAM secret key, so the IAM user needs `ses:SendRawEmail` and nothing else to set up.
- **Verify the sender** in SES (same region). In the SES sandbox you can only send to verified addresses.
- **Local development:** keep `MAIL_DRIVER=log`, or run Mailpit and use `smtp` on port 1025.

## A mail class: `make:mail`

Loco has `cargo run -- generate mailer <Name>` for its own mailer, but that type has no attachments. For mail with attachments use the module generator:

```sh
cargo modular make:mail InvoiceReady --module=billing
```

It creates the class and its HTML and text templates in the module, registers them, and adds the `module-mail` dependency.

## Try the whole flow

```sh
cargo run -- task mail:send-invoice to:you@example.com                   # dry run to a file with MAIL_DRIVER=log
MAIL_DRIVER=ses cargo run -- task mail:send-invoice to:you@example.com   # really send it
```

It makes the invoice PDF with Chrome, emails it as an attachment, and prints the log row and SES's message id.

The full walkthrough, with Loco's own mailer alongside, screenshots and SES setup, is [tutorial 24](24-email-and-pdf-step-by-step.md).

<!-- foot:start -->
---
[Previous: 22 Activity log, queue monitor and database backups](22-activity-log-queue-and-backups.md) | [Index](00-index.md) | [Next: 24 Email and PDF step by step](24-email-and-pdf-step-by-step.md)
<!-- foot:end -->
