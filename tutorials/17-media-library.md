<!-- nav:start -->
**Tutorial 17 of 17** | [Index](00-index.md) | [Previous: 16 The admin dashboard (`admin-layout` module)](16-admin-dashboard.md) | Next: none

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

</details>
<!-- nav:end -->

# 17. Media library (`media` module)

Modelled on Spatie's laravel-medialibrary (the version in Bimafy's `vendor/`). Any model can own files, grouped into **collections**, and there are two kinds of collection.

| Kind | Behaviour | Example |
|---|---|---|
| **Single file** | Adding a file replaces the old one (the old file is deleted) | Logo, avatar, cover |
| **Multiple files** | Keeps many files; `only_keep_latest(n)` trims the oldest | Gallery, attachments |

`only_keep_latest(1)` is the same as a single-file collection, exactly as in Spatie.

## Use it from a model

```rust
use module_media::{CollectionDef, HasMedia, InteractsWithMedia, Upload, IMAGES};

struct Post { id: i64 }

impl HasMedia for Post {
    fn media_type(&self) -> String { "post".into() }
    fn media_id(&self) -> i64 { self.id }
    fn media_collections(&self) -> Vec<CollectionDef> {          // like registerMediaCollections()
        vec![
            CollectionDef::single("cover").accepts_mime_types(IMAGES),
            CollectionDef::multiple("gallery").only_keep_latest(10).accepts_mime_types(IMAGES).public(),
        ]
    }
}

// upload an image
post.add_media(&ctx, Upload::new("photo.png", bytes))
    .using_name("Front photo")
    .with_custom_properties(json!({ "alt": "A photo" }))
    .to_media_collection("gallery")
    .await?;

let url = post.get_first_media_url(&ctx, "cover").await?;       // Some("/media/<uuid>/photo.png")
let all = post.get_media(&ctx, "gallery").await?;
post.clear_media_collection(&ctx, "gallery").await?;
post.delete_media(&ctx, media_id).await?;                        // only if it belongs to this model
```

No model type handy? Use `Owner::new("user", user_id).with_collections(vec![...])`; it implements the same trait.

| Spatie | Here |
|---|---|
| `addMedia($file)->toMediaCollection('x')` | `add_media(&ctx, upload).to_media_collection("x")` |
| `getMedia`, `getFirstMedia`, `getLastMedia` | `get_media`, `get_first_media`, `get_last_media` |
| `getFirstMediaUrl` | `get_first_media_url` |
| `hasMedia`, `clearMediaCollection`, `deleteMedia` | same names, snake_case |
| `registerMediaCollections()` | `media_collections()` |
| `singleFile()`, `onlyKeepLatest(n)`, `acceptsMimeTypes()` | `single(..)`, `only_keep_latest(n)`, `accepts_mime_types(..)` |
| `usingName`, `withCustomProperties`, `setOrder` | `using_name`, `with_custom_properties`, `set_order` |

Not built yet (Spatie has them): image conversions and thumbnails, responsive images, adding from a URL.

## Receiving an upload in a handler

```rust
async fn upload(State(ctx): State<AppContext>, Auth(user): Auth, multipart: Multipart) -> Result<Response> {
    let file = module_media::read_upload(multipart, "logo").await;   // the form field is called "logo"
    // ... then add_media(...).to_media_collection(...)
}
```

Form: `<form method="post" enctype="multipart/form-data"><input type="file" name="logo">`.

## What it checks for you

- **Content, not name:** the type is read from the first bytes (PNG, JPEG, GIF, WebP, PDF). A PHP file called `x.png` is rejected. SVG is not accepted because it can carry scripts.
- **Size:** each collection has `max_bytes` (default 2 MB, which is also Loco's request limit).
- **Names:** the stored name is sanitised, and the extension follows the content type.
- **Nothing is left behind** when a file is rejected or the database insert fails.

## Where files live and who can fetch them

- Files are stored through Loco's storage: `storage/uploads/` in development, in memory in tests. Point it at S3 or R2 later by changing the driver in `App::after_context`; no module code changes.
- Rows are in the `media_files` table (`model_type`, `model_id`, `collection_name`, `name`, `file_name`, `mime_type`, `size`, `custom_properties`, `order_column`, `is_public`).
- `GET /media/{uuid}/{file}` serves them. **Public** collections (`.public()`) go to everyone. **Private** ones (the default) go only to the owner and staff with `media.view`; everyone else gets a 404. Responses carry `nosniff` and a restrictive CSP.

## A worked example: the page feature image

Pages use the library exactly like any model would (`modules/pages/src/lib.rs`):

```rust
pub const fn feature_image_collection() -> CollectionDef {
    CollectionDef::single("feature_image").accepts_mime_types(IMAGES).max_bytes(DEFAULT_MAX_BYTES).public()
}

impl HasMedia for pages::Model {
    fn media_type(&self) -> String { "page".into() }
    fn media_id(&self) -> i64 { self.id }
    fn media_collections(&self) -> Vec<CollectionDef> { vec![feature_image_collection()] }
}
```

- **Edit it:** `/admin/pages/{id}/edit` (needs `pages.view` to look, `pages.manage` to change). Upload an image with alternative text, replace it, or remove it.
- **Public page:** `/pages/about` shows the image above the title, with the alt text, and adds `og:image` for sharing.
- **Cache:** the public page is cached for five minutes; uploading, removing, hiding or publishing clears it at once (`forget_cache`).
- **Hide a page:** the same screen can hide or publish a page.

## The media library page

`/admin/media` (needs `media.view`) is a **browser** for every file of every model, not a place to upload loose files. That matches Laravel, where a file always belongs to a model:

- Search by name; filter by collection and type; 24 per page.
- Each file shows its collection, size, date, public or private, and an **owner link** (for example "Page #3") that takes you to the screen where you manage it.
- Other uses: the account logo (`/account/logo`) is a single-file private collection.

## Receiving uploads in your own handler

```rust
let form = module_media::read_form(multipart, "image").await;   // file + the other text fields
let alt = form.fields.get("alt").cloned().unwrap_or_default();
let file = form.into_file()?;                                    // Err(Empty) or Err(TooLarge) explains why
```

Loco limits a request body to 2 MB. A bigger upload breaks the stream, and `into_file` reports that as `TooLarge` so the visitor sees "too large", not "nothing chosen".

<!-- foot:start -->
---
[Previous: 16 The admin dashboard (`admin-layout` module)](16-admin-dashboard.md) | [Index](00-index.md)
<!-- foot:end -->
