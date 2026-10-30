<!-- nav:start -->
**Tutorial 6 of 25** | [Index](00-index.md) | [Previous: 5 Modules and `cargo modular`](05-modules.md) | [Next: 7 Database: migrations, entities, queries, seeding](07-database.md)

<details><summary>Jump to any tutorial</summary>

[00. Index](00-index.md)  
01. [Introduction](01-introduction.md)  
02. [Setup](02-setup.md)  
03. [Rust essentials](03-rust-essentials.md)  
04. [Project structure](04-project-structure.md)  
05. [Modules and `cargo modular`](05-modules.md)  
**06. [Routes, controllers and views](06-routes-controllers-views.md)**  
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
23. [PDF and email](23-pdf-and-email.md)  
24. [Email and PDF step by step](24-email-and-pdf-step-by-step.md)  
25. [AI service: Claude, OpenAI and Gemini](25-ai-service.md)  

</details>
<!-- nav:end -->

# 06. Routes, controllers and views

## A request, step by step

```
browser ─► Axum router ─► handler (controller) ─► Tera template ─► HTML
                              │
                              └─► database / cache
```

## Routes

Routes are plain data, returned by each module:

```rust
pub fn routes() -> Vec<Routes> {
    vec![Routes::new().prefix("pages").add("/{slug}", get(show))]
}
```

`prefix("pages")` plus `/{slug}` gives `/pages/{slug}`. `{slug}` is a path parameter.

## A handler

```rust
#[debug_handler]
async fn show(
    ViewEngine(v): ViewEngine<TeraView>,   // the template engine
    State(ctx): State<AppContext>,         // database, cache, config
    Path(slug): Path<String>,              // from the URL
    locale: Locale,                        // our own extractor
) -> Result<Response> {
    // ...
    format::render().view(&v, "pages/show.html", data!({ "page": page }))
}
```

The parameters are **extractors**. Axum reads the request and builds each one for you, the way Laravel injects `Request` or route-bound models. You can write your own extractor by implementing `FromRequestParts` (see `modules/site-layout/src/locale.rs`).

The return type is `Result<Response>`. Returning `Err(...)` becomes an HTTP error.

## Returning other responses

```rust
Ok((StatusCode::NOT_FOUND, Html(body)).into_response())            // HTML with a status
(StatusCode::SEE_OTHER, [(header::LOCATION, "/")]).into_response()  // redirect
```

## Tera templates

Tera syntax is close to Jinja or Twig:

```html
{% extends "layouts/site.html" %}
{% block title %}{{ page.title }} | {{ t(key="site-name", lang=tag) }}{% endblock title %}
{% block content %}
  <h1>{{ page.title }}</h1>
  <div class="page-body">{{ page.body_html | safe }}</div>
{% endblock content %}
```

- `{{ value }}` prints and **escapes** HTML.
- `| safe` prints raw HTML. Use it only for content you trust (staff-written page bodies).
- `{% if lang == "bn" %}...{% else %}...{% endif %}`, `{% for item in items %}`.
- `t(key="...", lang=tag)` is the translation function (see [08](08-bilingual-site.md)).
- Pass data with `data!({ "name": value })`; values must be serialisable (`#[derive(Serialize)]`).

Tera 2 resolves `{% extends %}` when templates are added, so the engine loads all module templates in one batch (see `src/initializers/view_engine.rs`).

## HTMX in one minute

HTMX lets HTML elements call the server and swap in the returned HTML, with no JavaScript written by you:

```html
<input type="search" name="q"
       hx-get="/admin/pages" hx-trigger="keyup changed delay:300ms"
       hx-target="#results">
<div id="results">...</div>
```

The handler returns only the table fragment when the `HX-Request` header is present, and the full page otherwise. This is the Livewire-style approach planned for the admin. Add the script tag to the admin layout when that module is built.

<!-- foot:start -->
---
[Previous: 5 Modules and `cargo modular`](05-modules.md) | [Index](00-index.md) | [Next: 7 Database: migrations, entities, queries, seeding](07-database.md)
<!-- foot:end -->
