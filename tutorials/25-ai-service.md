<!-- nav:start -->
**Tutorial 25 of 28** | [Index](00-index.md) | [Previous: 24 Email and PDF step by step](24-email-and-pdf-step-by-step.md) | [Next: 26 Commands and jobs for Laravel developers](26-commands-and-jobs-for-laravel-developers.md)

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
19. [Components and the contact form](19-components-and-contact-form.md)  
20. [Tinker and sharing the database with Laravel](20-tinker-and-laravel-database.md)  
21. [Conventions](21-conventions.md)  
22. [Activity log, queue monitor and database backups](22-activity-log-queue-and-backups.md)  
23. [PDF and email](23-pdf-and-email.md)  
24. [Email and PDF step by step](24-email-and-pdf-step-by-step.md)  
**25. [AI service: Claude, OpenAI and Gemini](25-ai-service.md)**  
26. [Commands and jobs for Laravel developers](26-commands-and-jobs-for-laravel-developers.md)  
27. [PostgreSQL for MySQL users](27-postgresql-for-mysql-users.md)  
28. [MySQL or PostgreSQL, one choice per app](28-mysql-or-postgresql-per-app.md)  

</details>
<!-- nav:end -->

# 25. AI service: one wrapper for Claude, OpenAI and Gemini

The `ai-service` module is the only place that talks to AI providers. A feature (reading a passport photo, translating a page, making a cover picture) calls the wrapper; the wrapper picks the provider and model from `.env`, sends the request, logs the call and returns the answer. Swapping a provider is a `.env` change, not a code change.

| You want | Call | Provider |
|---|---|---|
| An answer to a question | `ai.chat(Request::new("..."))` | Claude, OpenAI or Gemini |
| A photo read into data | `ai.json(Request::new("...").image("image/jpeg", bytes))` | any of the three (they all read pictures) |
| A picture made | `ai.generate_image(ImageRequest::new("..."))` | OpenAI |

## 1. Put keys and models in `.env`

```text
AI_DEFAULT_PROVIDER=claude          # claude | openai | gemini; empty = the first one that has a key
ANTHROPIC_API_KEY=...
CLAUDE_MODEL=...
OPENAI_API_KEY=...
OPENAI_MODEL=...
OPENAI_IMAGE_MODEL=...
GEMINI_API_KEY=...
GEMINI_MODEL=...
```

Model names change often, so you write them; nothing is hard-wired. `AI_DEFAULT_MODEL` overrides the model of the default provider only. Then look:

```sh
cargo run -- task ai-service:status        # default provider, key set or missing, model per provider (keys never printed)
cargo run -- task ai-service:ask prompt:"Say hi in Bangla"
cargo run -- task ai-service:generate-image prompt:"a red bicycle" size:1024x1024
```

## 2. Call it from your code

```rust
use module_ai_service::{AiService, ImageRequest, Provider, Request};

let ai = AiService::new(&ctx)?;

let reply = ai.chat(Request::new("Translate to Bangla: Good morning").purpose("translate-page")).await?;
println!("{} ({} tokens, about ${:?})", reply.text, reply.total_tokens(), reply.cost);
```

Add the dependency in your module's `Cargo.toml`: `module-ai-service = { path = "../ai-service" }`. A request can name its own provider and model: `.provider(Provider::Gemini).model("...")`. Always give a `purpose`; it is what you search for later in the AI API logs.

## 3. Pictures in, data out (passports, device labels, receipts)

```rust
let (passport, _reply) = ai
    .json(
        Request::new("Read this passport page. Return JSON with surname, given_names, number, date_of_birth, expiry_date.")
            .image("image/jpeg", photo_bytes)
            .purpose("passport-scan")
            .sensitive(),
    )
    .await?;
let number = passport["number"].as_str().ok_or_else(|| Error::string("no passport number found"))?;
```

`json` asks each provider for JSON in its own way and still copes when the model wraps the answer in a markdown fence or a sentence. Check the fields you need: a model's answer is input, not truth. **`sensitive()`** means the prompt and the reply are not written to the AI API logs. Pictures are never stored, sensitive or not.

## 4. The AI API logs

Every call is a row in `ai_api_logs`, also the ones that failed (wrong key, rate limit, timeout, an answer that is not JSON, a blocked answer). Open **System, AI API logs** (`/admin/ai-api-logs`):

- Filter by status, provider, kind (`text`, `vision`, `image`) or text; open a row for the prompt, reply, tokens, cost and the provider's own error message.
- Choose a **page length** of 25, 50, 100, 200 or **500**, tick the **select-all** box, press **Delete selected** and confirm. **Delete all N matching** removes every page of the current filter; **Delete old** removes everything older than N days.
- `ai-service:purge-ai-api-logs` does the same on a schedule (daily, 30 days).
- Viewing needs `ai_api_logs.view`; deleting needs `ai_api_logs.manage`.

Set `AI_LOG_CONTENT=false` to keep prompts and replies out of every row; sizes, status and cost are still stored. To show an estimated cost, add prices per million tokens (for example `CLAUDE_INPUT_USD_PER_MILLION_TOKENS`).

## 5. Tests without a real provider

Point a provider's `base_url` at a small local server that returns a canned answer, and build the service with `AiService::with_config(db, config)`. `tests/tasks/ai_service.rs` does this for all three providers, images, failures and sensitive calls; no key and no network are needed.

## Checklist for a feature that uses AI

- [ ] calls `AiService`, never a provider directly
- [ ] a clear `purpose`
- [ ] `.sensitive()` when the content is about a person
- [ ] the answer is validated before it is stored
- [ ] a test with a canned provider answer, including a failing one

<!-- foot:start -->
---
[Previous: 24 Email and PDF step by step](24-email-and-pdf-step-by-step.md) | [Index](00-index.md) | [Next: 26 Commands and jobs for Laravel developers](26-commands-and-jobs-for-laravel-developers.md)
<!-- foot:end -->
