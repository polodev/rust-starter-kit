# Tutorials: Rust starter kit

Start here. Every page has an **Index** link at the top and bottom, plus Previous and Next, so you can always get back to this page.

## All tutorials

| # | Page | What you learn |
|---|---|---|
| 01 | [Introduction](01-introduction.md) | What the kit is and how each part maps to Laravel |
| 02 | [Setup](02-setup.md) | Install the tools, create the databases, run the site |
| 03 | [Rust essentials](03-rust-essentials.md) | The Rust you need to read and write this code |
| 04 | [Project structure](04-project-structure.md) | Workspace, crates and where things live |
| 05 | [Modules and `cargo modular`](05-modules.md) | `cargo modular` and how a module is built |
| 06 | [Routes, controllers and views](06-routes-controllers-views.md) | Requests in, HTML out: routes, handlers, Tera, HTMX |
| 07 | [Database: migrations, entities, queries, seeding](07-database.md) | Migrations, entities, queries and seeding on MariaDB |
| 08 | [A bilingual site (English and Bangla)](08-bilingual-site.md) | English and Bangla, end to end |
| 09 | [Cache and sessions](09-cache-and-sessions.md) | Short-lived cache and long-lived cookies |
| 10 | [Testing](10-testing.md) | Writing and running tests safely |
| 11 | [Assets and styling](11-assets-and-styling.md) | Tailwind, fonts and static files |
| 12 | [Cheat sheet and what to build next](12-cheatsheet.md) | Laravel to Rust quick reference and what to build next |
| 13 | [Console commands and the scheduler](13-console-and-schedule.md) | Make commands, schedule them, run the scheduler |
| 14 | [Login, register and sessions (`auth` module)](14-auth-and-sessions.md) | Login, register, cookie sessions and their safety checks |
| 15 | [Roles and permissions (`role-permission` module)](15-roles-and-permissions.md) | The six-role hierarchy and how permissions are checked |
| 16 | [The admin dashboard (`admin-layout` module)](16-admin-dashboard.md) | The admin shell, sidebar menu, navigation hub and quick search |
| 17 | [Media library (`media` module)](17-media-library.md) | Collections, `HasMedia`, uploads, Spatie-compatible table |
| 18 | [JSON API and Google login](18-json-api-and-google-login.md) | Bearer tokens, `ApiAuth`, sign in with Google |
| 19 | [Components and the contact form](19-components-and-contact-form.md) | Update a form without a reload, honeypot, admin modal |
| 20 | [Tinker and sharing the database with Laravel](20-tinker-and-laravel-database.md) | SQL and Eloquent consoles, Laravel gotchas |
| 21 | [Conventions](21-conventions.md) | The rules of this starter pack, short version |
| 22 | [Activity log, queue monitor and database backups](22-activity-log-queue-and-backups.md) | Audit trail, job monitor, `backup:run --only-db` |
| 23 | [PDF and email](23-pdf-and-email.md) | Headless Chrome PDFs, email through Amazon SES, `make:mail` |
| 24 | [Email and PDF step by step](24-email-and-pdf-step-by-step.md) | Loco's mailer and our `mail` module side by side, an invoice PDF by email, SES setup |
| 25 | [AI service](25-ai-service.md) | One wrapper for Claude, OpenAI and Gemini, photos to JSON, image generation, AI API logs |

## Pick a path

| I want to... | Read |
|---|---|
| Run the site and look around | [02](02-setup.md), then open `http://localhost:5150` |
| Learn just enough Rust | [03](03-rust-essentials.md) |
| Understand how the project is laid out | [04](04-project-structure.md), [05](05-modules.md) |
| Add a page, a table or a feature | [05](05-modules.md), [06](06-routes-controllers-views.md), [07](07-database.md) |
| Add a language or change text | [08](08-bilingual-site.md) |
| Run a task on a schedule | [13](13-console-and-schedule.md) |
| Add login, roles or an admin page | [14](14-auth-and-sessions.md), [15](15-roles-and-permissions.md), [16](16-admin-dashboard.md) |
| Let a model own uploaded images | [17](17-media-library.md) |
| Build a JSON API or Google sign-in | [18](18-json-api-and-google-login.md) |
| Add a form that updates without a reload | [19](19-components-and-contact-form.md) |
| Change the database from a prompt, or share it with Laravel | [20](20-tinker-and-laravel-database.md) |
| See who did what, watch jobs, back up the database | [22](22-activity-log-queue-and-backups.md) |
| Call Claude, OpenAI or Gemini, read a photo into data, make an image | [25](25-ai-service.md) |
| Make a PDF, send an email with SES, `make:mail` | [23](23-pdf-and-email.md), step by step: [24](24-email-and-pdf-step-by-step.md) |
| Know the rules before changing code | [21](21-conventions.md), `../docs/CONVENTIONS.md` |
| Look something up fast | [12](12-cheatsheet.md) |

## Related files

- [`../README.md`](../README.md): quick start and the list of modules
- [`../docs/CONVENTIONS.md`](../docs/CONVENTIONS.md): the rulebook for people and coding agents
