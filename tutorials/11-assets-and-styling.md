<!-- nav:start -->
**Tutorial 11 of 25** | [Index](00-index.md) | [Previous: 10 Testing](10-testing.md) | [Next: 12 Cheat sheet and what to build next](12-cheatsheet.md)

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
**11. [Assets and styling](11-assets-and-styling.md)**  
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

# 11. Assets and styling

## Static files

Everything in `assets/static/` is served under `/static`. For example `assets/static/css/app.css` is `/static/css/app.css`. The layout links it.

## Tailwind without Node

The standalone Tailwind CLI lives in `tools/tailwindcss` (git-ignored).

| File | Role |
|---|---|
| `assets/css/input.css` | Your source: Tailwind layers plus custom components |
| `tailwind.config.js` | Which files to scan for class names |
| `assets/static/css/app.css` | Generated output; do not edit |

Build:

```sh
./tools/tailwindcss -c tailwind.config.js -i assets/css/input.css -o assets/static/css/app.css --minify
```

While designing, use `--watch` instead of `--minify` in a second terminal.

Tailwind only includes classes it finds in the scanned files. The config scans `modules/*/templates/**/*.html`, so a class used in a module template is picked up after a rebuild. If a new class has no effect, rebuild the CSS.

## Reusable styles

Repeated groups of classes belong in `input.css` under `@layer components`:

```css
@layer components {
  .page-body h2 { @apply mt-8 text-xl font-semibold text-slate-900; }
}
```

The `page-body` class styles the staff-written HTML of a page without adding classes inside the database content.

## Fonts

`layouts/site.html` loads Inter and Noto Sans Bengali from Google Fonts and sets `font-latin` or `font-bangla` on `<body>` by language. For a site with strict privacy needs, download the fonts and serve them from `assets/static/fonts/`.

## Changing the look

Edit `modules/site-layout/templates/layouts/site.html`. Every page extends it, so the header, footer and fonts change everywhere at once.

<!-- foot:start -->
---
[Previous: 10 Testing](10-testing.md) | [Index](00-index.md) | [Next: 12 Cheat sheet and what to build next](12-cheatsheet.md)
<!-- foot:end -->
