<!-- nav:start -->
**Tutorial 16 of 21** | [Index](00-index.md) | [Previous: 15 Roles and permissions (`role-permission` module)](15-roles-and-permissions.md) | [Next: 17 Media library (`media` module)](17-media-library.md)

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
**16. [The admin dashboard (`admin-layout` module)](16-admin-dashboard.md)**  
17. [Media library (`media` module)](17-media-library.md)  
18. [JSON API and Google login](18-json-api-and-google-login.md)  
19. [Components and the contact form](19-components-and-contact-form.md)  
20. [Tinker and sharing the database with Laravel](20-tinker-and-laravel-database.md)  
21. [Conventions](21-conventions.md)  

</details>
<!-- nav:end -->

# 16. The admin dashboard (`admin-layout` module)

Sidebar sections, quick search and a navy top bar. The admin is English only and not indexed.

| URL | Needs | Shows |
|---|---|---|
| `/admin` | `dashboard.view` | Navigation hub: every link you may use, with search |
| `/admin/users`, `/{id}`, `/{id}/edit` | `users.view` / `users.manage` | List, read-only show, edit (name, email, profile image, generate password) |
| `/admin/role-permissions/...` | `roles.view` | Overview, roles, permissions, matrix, user assignments, backend links (tutorial 15) |
| `/admin/pages`, `/{id}`, `/{id}/edit` | `pages.view` / `pages.manage` | Pages: list, show, text editor per language, feature image |
| `/admin/media` | `media.view` / `media.manage` | Every file, direct upload (tutorial 17) |
| `/admin/contacts` | `contacts.view` / `contacts.manage` | Inbox with a modal and a replied status (tutorial 19) |
| `/admin/schedule` | `queue.view` | Scheduled jobs |

A guest is sent to login and returned afterwards. Someone without the permission sees "Access denied" inside the shell.

## How every admin page looks

- **One container:** the layout wraps every page in one wide container. Do not add your own `max-w-*`.
- **Breadcrumb:** `Dashboard › Users › Demo User › Edit`, set in `{% block breadcrumb %}`. If you do not override it the layout shows `Dashboard › {{ title }}`. No "back" links.
- **Lists:** the first column is **ID** and links to the show page. A right-aligned Actions column has **Show** and, if permitted, **Edit**.
- **Show** is read-only with an Edit button; **Edit** is its own page. Saving redirects with `?saved=...`, errors with `?error=...`.
- **Finding things:** the sidebar shows only what you may use; **Ctrl/Cmd + K** searches every item.

## Add your page

In your module, add a sidebar link:

```rust
fn admin_menu(&self) -> Vec<MenuItem> {
    vec![MenuItem {
        section: "Content", label: "Pages", href: "/admin/pages",
        permission: "pages.view",              // without it the link is hidden
        icon: "fa-regular fa-file-lines", order: 1, match_children: true,
    }]
}
```

Then a handler and a template:

```rust
#[debug_handler]
async fn index(ViewEngine(v): ViewEngine<TeraView>, page: AdminPage) -> Result<Response> {
    if !page.can("pages.view") { return page.forbidden(&v); }
    page.render(&v, "admin/pages.html", json!({ "title": "Pages", "rows": rows }))
}
```

```html
{% extends "layouts/admin.html" %}
{% block breadcrumb %}<nav ...>Dashboard › Pages</nav>{% endblock breadcrumb %}
{% block content %}<h1>{{ title }}</h1>{% endblock content %}
```

`AdminPage` redirects guests to login and builds the sidebar. Always pass a `title`. Form posts call `module_auth::same_origin(&headers)` first. The full rules and a checklist are in `docs/CONVENTIONS.md`.

<!-- foot:start -->
---
[Previous: 15 Roles and permissions (`role-permission` module)](15-roles-and-permissions.md) | [Index](00-index.md) | [Next: 17 Media library (`media` module)](17-media-library.md)
<!-- foot:end -->
