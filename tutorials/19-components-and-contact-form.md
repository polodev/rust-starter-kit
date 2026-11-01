<!-- nav:start -->
**Tutorial 19 of 28** | [Index](00-index.md) | [Previous: 18 JSON API and Google login](18-json-api-and-google-login.md) | [Next: 20 Tinker and sharing the database with Laravel](20-tinker-and-laravel-database.md)

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
**19. [Components and the contact form](19-components-and-contact-form.md)**  
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

# 19. Components and the contact form (`contact` module)

Laravel Volt/Livewire lets a form update without a page load. Here a **component** is a server-rendered Tera partial plus a 25-line script. No framework.

## How a component works

1. The page includes the partial: `{% include "contact/_form.html" %}`. Its root has `data-component-root`; its form has `data-component`.
2. `assets/static/js/components.js` posts the form with `fetch` and the header `X-Component: 1`.
3. The handler checks that header. With it, it returns **only the partial**; without it, the **whole page**.
4. The script replaces the old partial with the new one and fires `component:updated`.

```rust
fn page(v: &TeraView, site: &Site, headers: &HeaderMap, status: StatusCode, data: Value) -> Result<Response> {
    let template = if wants_fragment(headers) { "contact/_form.html" } else { "contact/page.html" };
    Ok((status, Html(v.render(template, site.data(data))?)).into_response())
}
```

Because the no-header path returns a full page, the form still works with JavaScript off.

## The public form (`/contact`)

- **Honeypot:** a field called `website`, hidden off-screen. If it is filled, answer exactly like success and store nothing.
- **Origin check:** a post from another site gets 403. **Rate limit:** 5 messages per 10 minutes per IP (cache). Both are cheap, neither stops a determined bot; add CAPTCHA if spam grows.
- **Validation:** length and email checked on the server; errors come back with the typed values kept (422).
- **Text:** Fluent keys `contact-*` in English and Bangla.

## The admin inbox (`/admin/contacts`)

- List with search and an **All / Not replied / Replied** filter; ID links to the message.
- Click a row (or its ID): an Alpine modal fetches `GET /admin/contacts/{id}` with `X-Component: 1` and shows the message.
- **Mark as replied** is a `data-component` form inside the modal. After it, the modal and the list badge update with no reload.
- `contacts.view` reads, `contacts.manage` changes the status.

| Route | Purpose |
|---|---|
| `GET /admin/contacts` | list |
| `GET /admin/contacts/{id}` | one message (modal fragment, or a page) |
| `POST /admin/contacts/{id}/status` | `new` or `replied` |

To make your own component: copy `contact/_form.html`, give the root `data-component-root` and the form `data-component`, and make the handler return the partial when `wants_fragment`.

<!-- foot:start -->
---
[Previous: 18 JSON API and Google login](18-json-api-and-google-login.md) | [Index](00-index.md) | [Next: 20 Tinker and sharing the database with Laravel](20-tinker-and-laravel-database.md)
<!-- foot:end -->
