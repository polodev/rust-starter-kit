<!-- nav:start -->
**Tutorial 9 of 25** | [Index](00-index.md) | [Previous: 8 A bilingual site (English and Bangla)](08-bilingual-site.md) | [Next: 10 Testing](10-testing.md)

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
**09. [Cache and sessions](09-cache-and-sessions.md)**  
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

# 09. Cache and sessions

The kit uses two kinds of "remembering".

| | Short-lived | Long-lived |
|---|---|---|
| Purpose | Remember a database result | Remember a person |
| Laravel idea | `Cache::remember` | Session and "remember me" |
| Lifetime | Seconds to minutes | Days to a year |
| Stored | Server memory (Redis later) | Browser cookie, plus database for login |
| Built | Yes | Language cookie yes, login not yet |

## Short-lived: `remember`

```rust
use loco_modular::remember;

let page = remember(&ctx, &format!("pages:{slug}:{code}"), Duration::from_secs(300), async {
    find_page(&ctx.db, &slug, code).await
})
.await?;
```

It returns the cached value if present. Otherwise it runs the query, stores the result for the given time, and returns it.

- The value must be `Serialize + DeserializeOwned`.
- Build keys from everything that changes the result (`slug` and language here).
- If the cache itself fails, `remember` runs the query directly, so a cache outage never breaks a page.
- Cache what is cheap to be slightly stale: public pages, lookups, counts. Do not cache per-user secrets under a shared key.
- When you edit content in an admin, delete the matching key (`ctx.cache.remove(key)`) or wait for the time to pass.

Configuration in `config/*.yaml`:

```yaml
cache:
  kind: InMem
  max_capacity: 1000
```

For several servers, switch to Redis (`kind: Redis`) so they share one cache.

## Long-lived: cookies

The language choice is a cookie that lasts a year:

```
lang=bn; Path=/; Max-Age=31536000; SameSite=Lax
```

That is fine for a preference. A **login** needs more care, and it is the next piece to build:

- a random session id in a cookie marked `HttpOnly`, `Secure` and `SameSite=Lax`
- the session stored in the database (user id, expiry, last use), so it can be revoked
- a refresh on use, so active users stay signed in
- the language preference saved on the user record, so it follows them across devices

Never put the user id or any secret in a plain cookie, and never trust cookie values without checking them, as `normalize()` does for the language.

<!-- foot:start -->
---
[Previous: 8 A bilingual site (English and Bangla)](08-bilingual-site.md) | [Index](00-index.md) | [Next: 10 Testing](10-testing.md)
<!-- foot:end -->
