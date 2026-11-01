<!-- nav:start -->
**Tutorial 14 of 28** | [Index](00-index.md) | [Previous: 13 Console commands and the scheduler](13-console-and-schedule.md) | [Next: 15 Roles and permissions (`role-permission` module)](15-roles-and-permissions.md)

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
**14. [Login, register and sessions (`auth` module)](14-auth-and-sessions.md)**  
15. [Roles and permissions (`role-permission` module)](15-roles-and-permissions.md)  
16. [The admin dashboard (`admin-layout` module)](16-admin-dashboard.md)  
17. [Media library (`media` module)](17-media-library.md)  
18. [JSON API and Google login](18-json-api-and-google-login.md)  
19. [Components and the contact form](19-components-and-contact-form.md)  
20. [Tinker and sharing the database with Laravel](20-tinker-and-laravel-database.md)  
21. [Conventions](21-conventions.md)  
22. [Activity log, queue monitor and database backups](22-activity-log-queue-and-backups.md)  
23. [PDF and email](23-pdf-and-email.md)  
24. [Email and PDF step by step](24-email-and-pdf-step-by-step.md)  
25. [AI service: Claude, OpenAI and Gemini](25-ai-service.md)  
26. [Commands and jobs for Laravel developers](26-commands-and-jobs-for-laravel-developers.md)  
27. [PostgreSQL for MySQL users](27-postgresql-for-mysql-users.md)  
28. [MySQL or PostgreSQL, one choice per app](28-mysql-or-postgresql-per-app.md)  

</details>
<!-- nav:end -->

# 14. Login, register and sessions (`auth` module)

The `auth` module owns everything about who you are: login, register, logout, the session cookie and the middleware that reads it.

## Pages

| URL | What it does |
|---|---|
| `GET/POST /register` | Create an account. Sign-up signs you in. New accounts have **no role**. |
| `GET/POST /login` | Sign in. Accepts `?next=/some/page` and returns there afterwards. |
| `POST /logout` | Ends this session and clears the cookie. |
| `GET /auth/google/redirect` | Sign in with Google (shown on the login page when `GOOGLE_CLIENT_ID` is set; see [tutorial 18](18-json-api-and-google-login.md)). |

The header: guests see **Log in** (amber link) and **Register** (button); signed-in people see an avatar with a menu (Admin dashboard if they hold `dashboard.view`, My account, Log out).

Sign in, sign out and registration are written to the [activity log](22-activity-log-queue-and-backups.md).

## How a session works

1. Login creates a random 256-bit token and sets it as the `session` cookie (`HttpOnly`, `SameSite=Lax`, `Secure` in production, 30 days).
2. The database stores only the **SHA-256 of the token** in the `sessions` table, with the user, expiry and last use. A leaked table cannot be replayed as a login.
3. On every request a middleware (`modules/auth/src/middleware.rs`) reads the cookie, finds the live session, loads the person and their permissions, and puts them in the request. Using the site slides the expiry forward (written at most once an hour).
4. `auth:prune-sessions` deletes expired rows. It is scheduled hourly.

## Using it in your own handler

```rust
use module_auth::{Auth, OptionalAuth};

async fn secret(Auth(user): Auth) -> Result<Response> {        // not signed in? redirect to /login?next=...
    user.access.require("pages.manage")?;                       // 403 when missing
    ...
}

async fn maybe(OptionalAuth(user): OptionalAuth) { ... }       // user is Option<AuthUser>
```

`AuthUser` carries `id`, `name`, `email`, `session_id` and `access` (role and permissions). The shared page data (`Site`) also carries the person for the header.

## Safety built in

- **CSRF:** every form post must come from this site. `same_origin()` checks the `Origin` (or `Referer`) host against `Host`. A post with neither header, or from another site, gets 403. Together with `SameSite=Lax` this stops other sites posting as you.
- **Open redirects:** `next` must be a local path. `//host`, full URLs, backslashes and `/login`/`/register` fall back to `/`.
- **Brute force:** five failed logins for an email lock it for 15 minutes (cache-based), even for the right password.
- **No account guessing:** a wrong password and an unknown email give the same message, and an unknown email still pays the cost of one password hash.
- **Passwords:** hashed with Argon2 (Loco). At least 8 characters.

## Email links: verify, reset, sign in

Three one-time links go through the `mail` module (all text is Fluent keys, so mails follow the visitor's language):

| Link | Pages | Lifetime |
|---|---|---|
| Verify the email address | sign-up sends it; `/email/verification-notification` asks again; `/email/verify/{secret}` | 24 hours |
| Reset a forgotten password | `/forgot-password`, `/reset-password/{secret}` | 60 minutes |
| Sign in without a password | `/login/email-link`, `/login/email-link/{secret}` | 15 minutes |

- Like Laravel: a new account is signed in at once but **not verified**. Handlers that take `VerifiedAuth` (the admin area and the account pages except the profile) send an unverified person to `/email/verify`, the notice page with a "send again" button. `Auth` only needs a session; `VerifiedAuth` is `Auth` plus the `verified` check.
- Asking for a link never reveals whether an address has an account, and one address gets at most 3 mails an hour per kind.
- The link holds a random secret; the `user_security_tokens` table keeps only its SHA-256. A link works once and expires.
- A password reset signs the person out everywhere. A reset or a sign-in link also verifies the address, because it reached the mailbox.
- Opening a sign-in link only shows a button; pressing it signs in. Mail scanners that open links ahead of the person would otherwise use the link up.
- In development `MAIL_DRIVER=log` writes the mails as `.eml` files in `storage/mail`; open the file and copy the link. Use `smtp` or `ses` to send real mail.
- Two-factor sign-in is not part of the starter.

## Good to know

- The login lockout uses the in-memory cache, so with several servers use Redis so they share the counter.
- The demo account (`demo@gmail.com` / `secret123$`) exists in **development only**. It is created by `cargo run -- db seed` and is a developer.

<!-- foot:start -->
---
[Previous: 13 Console commands and the scheduler](13-console-and-schedule.md) | [Index](00-index.md) | [Next: 15 Roles and permissions (`role-permission` module)](15-roles-and-permissions.md)
<!-- foot:end -->
