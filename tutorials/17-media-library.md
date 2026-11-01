<!-- nav:start -->
**Tutorial 17 of 28** | [Index](00-index.md) | [Previous: 16 The admin dashboard (`admin-layout` module)](16-admin-dashboard.md) | [Next: 18 JSON API and Google login](18-json-api-and-google-login.md)

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
**17. [Media library (`media` module)](17-media-library.md)**  
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

# 17. Media library (`media` module)

Modelled on Spatie laravel-medialibrary, and it uses Spatie's own `media` table and `{id}/{file_name}` file layout, so a Laravel app can share the database and the bucket. Any model can own files grouped into **collections**.

| Kind | Behaviour | Example |
|---|---|---|
| **Single file** | Adding a file replaces the old one | Logo, avatar, feature image |
| **Multiple files** | Keeps many; `only_keep_latest(n)` trims the oldest | Gallery, attachments |

## Use it from a model

```rust
use module_media::{CollectionDef, HasMedia, InteractsWithMedia, Upload, IMAGES};

impl HasMedia for Post {
    fn media_type(&self) -> String { "post".into() }       // stored in model_type
    fn media_id(&self) -> i64 { self.id }
    fn media_collections(&self) -> Vec<CollectionDef> {    // registerMediaCollections()
        vec![
            CollectionDef::single("cover").accepts_mime_types(IMAGES),
            CollectionDef::multiple("gallery").only_keep_latest(10).accepts_mime_types(IMAGES).public(),
        ]
    }
}

post.add_media(&ctx, Upload::new("photo.png", bytes))
    .using_name("Front photo")
    .with_custom_properties(json!({ "alt": "A photo" }))
    .to_media_collection("gallery").await?;

let url = post.get_first_media_url(&ctx, "cover").await?;   // Some("/media/12/photo.png")
post.clear_media_collection(&ctx, "gallery").await?;
```

No model type handy? `Owner::new("user", id).with_collections(vec![...])` implements the same trait. Method names are Spatie's in snake_case (`get_media`, `get_first_media`, `delete_media`, `using_name`, `set_order`).

In a handler: `let form = module_media::read_form(multipart, "image").await; let file = form.into_file()?;` then `add_media(...)`.

## What it checks

- **Content, not name:** PNG, JPEG, GIF, WebP and PDF are recognised from the first bytes. SVG is refused (it can carry scripts).
- **Size:** `max_bytes` per collection (default 10 MB). The request limit is `limit_payload` in `config/*.yaml` (11 MB); raise both together, and your proxy's limit too.
- **Names:** sanitised, with the extension following the content type. Nothing is left behind when a file is rejected.

## Where files live and who can fetch them

- **Storage:** Loco's storage (`storage/uploads/` in development; S3 or R2 by config). A file is at `{id}/{file_name}`, with `settings.media.prefix` in front when set.
- **Table:** `media` with Spatie's columns (`model_type`, `model_id` as text, `uuid`, `collection_name`, `disk`, `size`, `custom_properties`, ...).
- **URL:** `GET /media/{id}/{file_name}`. **Public** collections (`.public()`) go to everyone; the default is **private**: only the owner and staff with `media.view`, everyone else gets 404.
- **Cloudflare R2:** set `MEDIA_STORAGE=r2`, `MEDIA_DISK=r2-public` and `MEDIA_PREFIX=<folder>` in `.env` (the `R2_*` keys, endpoint and `R2_PUBLIC_BUCKET` are already there) and restart: uploads go to the public bucket instead of `storage/uploads`. Files uploaded earlier are copied with `cargo run -- task media:move-local-uploads` (try `dry-run:true` first; the local copies are kept). With the bucket's public address in `R2_PUBLIC_URL` (the `pub-....r2.dev` link from Cloudflare, bucket settings, Public access), public files load straight from R2; without it they are served through this app at `/media/{id}/{file_name}`.
- **Warning:** every collection, private ones too, goes to this one bucket. Once its public access is on, anyone who guesses `{prefix}/{id}/{file_name}` can read the file, so only upload public collections until private ones have their own private bucket. The r2.dev address is rate-limited and meant for development; use a custom domain in production.
- **Warning:** today every collection, private ones too, goes to this one bucket. That is safe while the bucket's public access is off. Do not switch public access on until private collections have their own private bucket.
- **Public address:** set `R2_PUBLIC_URL` (the bucket's public domain) or `settings.media.disk_urls.<disk>`, and public files redirect to `{url}/{path}` so the bytes skip this app.

```yaml
settings:
  media:
    disk: s3-media        # written to new rows
    prefix: ""
    disk_urls:
      s3-media: https://cdn.example.com
```

## Admin

- **`/admin/media`** (needs `media.view`): every file of every model, searchable and filterable. ID links to the file; thumbnails open in a lightbox; the entity links to its record.
- **Direct upload** (needs `media.manage`): drag and drop several files; each is sent in its own request and checked on its own. Loose files belong to the built-in owner `library` (id 0), are public, and can be deleted here. A file that belongs to a record is changed on that record.
- **Examples:** a page's feature image (`modules/pages`) and a user's profile image (`logo` collection, edited on `/admin/users/{id}/edit` and `/account/logo`).

## Sharing the database with Laravel

- **Owners:** use the exact `model_type` Laravel stores (`Owner::new("enrollment", 77)`). Uuid keys are readable too; only numeric keys give an `owner_id()`.
- **Import:** `cargo run -- task media:import database:legacy_app` copies another database's `media` table, keeping ids; run it again to pick up new rows. If both apps use one database there is nothing to import.
- **Imported files are private** until `custom_properties` has `"public": true`.
- **Not built:** image conversions and thumbnails, responsive images, adding from a URL. Their columns are kept and written as `[]`.

<!-- foot:start -->
---
[Previous: 16 The admin dashboard (`admin-layout` module)](16-admin-dashboard.md) | [Index](00-index.md) | [Next: 18 JSON API and Google login](18-json-api-and-google-login.md)
<!-- foot:end -->
