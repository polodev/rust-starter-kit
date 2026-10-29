# 16. The admin dashboard (`admin-layout` module)

Style follows eng4 (sidebar sections, quick search) with Dokani's navy top bar. The admin is English only and not indexed by search engines.

| URL | Needs | Shows |
|---|---|---|
| `/admin` | `dashboard.view` | Stat cards and the newest users |
| `/admin/users` | `users.view` | All users with their role; developers can change roles |
| `/admin/roles` | `roles.view` | Roles and the permission matrix; developers can sync |
| `/admin/schedule` | `queue.view` | Scheduled jobs from modules and config |
| `/admin/media` | `media.view` | The media library (see tutorial 17) |

A guest is sent to the login page and returned after signing in. A signed-in person without the permission sees an "Access denied" page inside the admin shell.

## Finding things easily

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

Next: [17-media-library.md](17-media-library.md)
