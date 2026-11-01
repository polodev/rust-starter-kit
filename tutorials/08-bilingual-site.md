<!-- nav:start -->
**Tutorial 8 of 28** | [Index](00-index.md) | [Previous: 7 Database: migrations, entities, queries, seeding](07-database.md) | [Next: 9 Cache and sessions](09-cache-and-sessions.md)

<details><summary>Jump to any tutorial</summary>

[00. Index](00-index.md)  
01. [Introduction](01-introduction.md)  
02. [Setup](02-setup.md)  
03. [Rust essentials](03-rust-essentials.md)  
04. [Project structure](04-project-structure.md)  
05. [Modules and `cargo modular`](05-modules.md)  
06. [Routes, controllers and views](06-routes-controllers-views.md)  
07. [Database: migrations, entities, queries, seeding](07-database.md)  
**08. [A bilingual site (English and Bangla)](08-bilingual-site.md)**  
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
26. [Commands and jobs for Laravel developers](26-commands-and-jobs-for-laravel-developers.md)  
27. [PostgreSQL for MySQL users](27-postgresql-for-mysql-users.md)  
28. [MySQL or PostgreSQL, one choice per app](28-mysql-or-postgresql-per-app.md)  

</details>
<!-- nav:end -->

# 08. A bilingual site (English and Bangla)

Two kinds of text, handled two ways:

| Kind | Example | Where it lives |
|---|---|---|
| Interface strings | nav labels, buttons, footer | Fluent files in `assets/i18n/` |
| Content | About, Privacy Policy, Terms | Database table `page_translations` |

## Which language is shown?

`Locale` (in `modules/site-layout/src/locale.rs`) is an Axum extractor. It decides in this order:

1. `?lang=bn` in the URL
2. the `lang` cookie
3. the `Accept-Language` header, honouring quality values
4. English

Any handler can take `locale: Locale` and call `locale.code()` (`"en"` or `"bn"`).

## Switching language

`GET /language/bn?next=/pages/about` sets the cookie and redirects back:

```
HTTP/1.1 303 See Other
set-cookie: lang=bn; Path=/; Max-Age=31536000; SameSite=Lax
location: /pages/about
```

The `next` value must start with a single `/`. Anything else (`//evil.example`, `https://...`) is ignored and the user goes to `/`. This prevents an open redirect.

## Interface strings (Fluent)

`assets/i18n/en-US/main.ftl`:

```
nav-about = About
home-title = Welcome to your new website
```

`assets/i18n/bn-BD/main.ftl`:

```
nav-about = আমাদের সম্পর্কে
home-title = আপনার নতুন ওয়েবসাইটে স্বাগতম
```

In a template: `{{ t(key="nav-about", lang=tag) }}`. The handler passes `tag` as `en-US` or `bn-BD` using `fluent_tag(code)`.

Rules:

- Add every key to **both** files. A test (`every_translation_key_exists_in_both_languages`) fails if they differ.
- Put shared terms starting with `-` in `assets/shared.ftl`, outside `assets/i18n/`.

## Page content

Each page has one row in `pages` and one row per language in `page_translations`:

```
pages:              id=1  slug=about
page_translations:  page_id=1  locale=en  title=About us
                    page_id=1  locale=bn  title=আমাদের সম্পর্কে
```

**English is required; every other language is optional.** In the admin editor an English title and text must be filled in. A Bangla title, description or text can be left empty: it is stored as `NULL` and visitors reading Bangla see the English text for just that part (the three parts fall back on their own). Clearing all Bangla fields removes the Bangla row, and the page is simply English. A half-translated page never breaks.

To add a language later (for example Hindi): add `hi` to `SUPPORTED_LOCALES`, create `assets/i18n/hi-IN/main.ftl`, map it in `fluent_tag`, and add translations.

## Bangla typography

The layout loads **Noto Sans Bengali** and switches the body font class when `lang == "bn"`. Always test pages in Bangla: text is wider and may need different spacing.

## Try it

```sh
curl -s "http://localhost:5150/pages/about?lang=bn" | grep "<h1"
curl -s -H "Accept-Language: bn-BD" http://localhost:5150/pages/about | grep "<h1"
curl -s -i "http://localhost:5150/language/bn?next=/pages/about"
```

<!-- foot:start -->
---
[Previous: 7 Database: migrations, entities, queries, seeding](07-database.md) | [Index](00-index.md) | [Next: 9 Cache and sessions](09-cache-and-sessions.md)
<!-- foot:end -->
