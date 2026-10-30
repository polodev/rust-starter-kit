# mail

Sending email: **Amazon SES** (through its SMTP interface) or any SMTP server, HTML and text bodies, attachments
(for example a PDF from the `pdf` module), and a `mail_log` table that records every attempt.

```rust
use module_mail::Mail;

let mail = Mail::to("customer@example.com")
    .subject("Your invoice")
    .html_template(include_str!("../templates/invoice.html"), &data)?
    .text_template(include_str!("../templates/invoice.txt"), &data)?
    .attach_pdf("invoice.pdf", pdf_bytes);
module_mail::send(&ctx, mail).await?;       // logged as queued, then sent or failed
```

`cc`, `bcc`, `reply_to`, `also_to` and `attach(name, content_type, bytes)` work as you would expect. A mail that cannot
be built (no recipient, bad address, empty subject, no body) is refused before anything is logged or sent.

## Make a mail class (like Laravel's `make:mail`)

```sh
cargo modular make:mail InvoiceReady --module=billing
```

creates `src/mails/invoice_ready.rs` (a struct with `build()`), `templates/mails/invoice_ready.html` and `.txt`,
registers it, and adds the `module-mail` dependency. Then `module_mail::send(&ctx, InvoiceReady { to, name }.build()?)`.

## Settings (`.env`)

```sh
MAIL_DRIVER=log                 # log | ses | smtp  (log writes .eml files to storage/mail and sends nothing)
MAIL_FROM_ADDRESS=support@example.com
MAIL_FROM_NAME=My App
AWS_SES_ACCESS_KEY_ID=...       # for MAIL_DRIVER=ses
AWS_SES_SECRET_ACCESS_KEY=...
AWS_SES_DEFAULT_REGION=ap-southeast-1
```

`config/development.yaml` and `config/production.yaml` read these into `settings.mail`.

### Amazon SES

- The driver talks to `email-smtp.<region>.amazonaws.com:587` (STARTTLS). SES wants the IAM **access key id** as the user
  name and a password *derived* from the IAM secret key and the region. The module does that derivation (AWS's documented
  algorithm, tested), so the IAM user's normal keys work as long as the user may call `ses:SendRawEmail`.
- The `From` address or domain must be **verified** in SES in that region. While the account is in the SES **sandbox**,
  mail can only go to verified addresses; ask AWS for production access to send to anyone.
- Optional `AWS_SES_CONFIGURATION_SET` adds the `X-SES-CONFIGURATION-SET` header, for delivery events.
- The provider's answer (the SES message id) is stored in `mail_log.provider_id`.

### Other SMTP servers

`MAIL_DRIVER=smtp` with `MAIL_SMTP_HOST`, `MAIL_SMTP_PORT`, `MAIL_SMTP_USERNAME`, `MAIL_SMTP_PASSWORD` and
`MAIL_SMTP_ENCRYPTION` (`none`, `starttls` or `tls`). For local development run a catcher such as Mailpit
(`docker run -p 1025:1025 -p 8025:8025 axllent/mailpit`) with host `localhost`, port `1025`, encryption `none`.

## Commands

```sh
cargo run -- task mail:send-test to:you@example.com
cargo run -- task mail:send-invoice to:you@example.com      # makes the PDF with Chrome, then emails it
MAIL_DRIVER=ses cargo run -- task mail:send-invoice to:you@example.com   # really send
```

## The log

`mail_log` has one row per attempt: recipients, subject, driver, `status` (`queued`, `sent`, `failed`), the error or the
provider's answer, attachment names and times. Tests use the `capture` driver, which keeps messages in memory
(`module_mail::captured()`).

## Not included yet

An admin screen for the log, automatic retry of failed mail, sending from a background worker, and SES bounce and
complaint handling (an SNS webhook).
