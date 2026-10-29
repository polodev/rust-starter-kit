<!-- nav:start -->
**Tutorial 16 of 17** | [Index](00-index.md) | [Previous: 15 Roles and permissions (`role-permission` module)](15-roles-and-permissions.md) | [Next: 17 Media library (`media` module)](17-media-library.md)

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

</details>
<!-- nav:end -->

# 16. The admin dashboard (`admin-layout` module)

Style follows eng4 (sidebar sections, quick search) with Dokani's navy top bar. The admin is English only and not indexed by search engines.

| URL | Needs | Shows |
|---|---|---|
| `/admin` | `dashboard.view` | Stat cards and the newest users |
| `/admin/users` | `users.view` | All users with their role; developers can change roles |
| `/admin/roles` | `roles.view` | Roles and the permission matrix; developers can sync |
| `/admin/schedule` | `queue.view` | Scheduled jobs from modules and config |
| `/admin/pages` | `pages.view` | Pages list: click a title to open it, **Edit** to change it |
| `/admin/pages/{id}` | `pages.view` | The page's **show page**: details, both languages as visitors see them, feature image, an **Edit** button |
| `/admin/pages/{id}/edit` | `pages.view` to look, `pages.manage` to change | Text editor per language, feature image, hide or publish |
| `/admin/media` | `media.view` to look, `media.manage` to upload and delete | A table of every file: thumbnails open in a lightbox, the entity links to its record; drag-and-drop upload (see tutorial 17) |

A guest is sent to the login page and returned after signing in. A signed-in person without the permission sees an "Access denied" page inside the admin shell.

## Finding things easily

- **The dashboard itself is a navigation hub** (like eng4). `/admin` opens on "All navigation links": a count of sections and links, a search box and every sidebar link as a small icon chip, grouped by section. It is built from the same menu as the sidebar, so it can never drift, and it only lists what you may use.
- The sidebar has sections (**Overview**, **Content**, **People and access**, **System**) and only shows links the person may use.
- The current page is highlighted.
- **Ctrl/Cmd + K** (or the "Find a page" button) opens a search over every sidebar item. Type, press Enter to open the first match.

## Adding your page to the sidebar

Each module contributes its own links. In your module:

```rust
fn admin_menu(&self) -> Vec<MenuItem> {
    vec![MenuItem {
        section: "Content",
        label: "Pages",
        href: "/admin/pages",
        permission: "pages.view",      // people without it never see the link
        icon: "fa-regular fa-file-lines",
        order: 1,
        match_children: true,
    }]
}
```

The app collects the menus of all modules (`loco_modular::collect_menu`) and the layout filters them per person. No template edit needed.

## Writing an admin page

```rust
use module_admin_layout::AdminPage;

#[debug_handler]
async fn index(ViewEngine(v): ViewEngine<TeraView>, page: AdminPage) -> Result<Response> {
    if !page.can("pages.view") {
        return page.forbidden(&v);
    }
    page.render(&v, "admin/pages.html", json!({ "title": "Pages", "rows": rows }))
}
```

```html
{% extends "layouts/admin.html" %}
{% block content %}
  <h1>{{ title }}</h1>
{% endblock content %}
```

`AdminPage` already redirects guests to the login page and builds the sidebar. Always pass a `title`. Form posts must call `module_auth::same_origin(&headers)` first.

<!-- foot:start -->
---
[Previous: 15 Roles and permissions (`role-permission` module)](15-roles-and-permissions.md) | [Index](00-index.md) | [Next: 17 Media library (`media` module)](17-media-library.md)
<!-- foot:end -->
