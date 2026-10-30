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
- **Size:** each collection has `max_bytes` (default 10 MB, which is also Loco's request limit).
- **Names:** the stored name is sanitised, and the extension follows the content type.
- **Nothing is left behind** when a file is rejected or the database insert fails.

## Where files live and who can fetch them

- Files are stored through Loco's storage: `storage/uploads/` in development, in memory in tests. Point it at S3 or R2 later by changing the driver in `App::after_context`; no module code changes.
- Rows are in the `media` table, which is Spatie laravel-medialibrary's own table (`model_type`, `model_id`, `uuid`, `collection_name`, `name`, `file_name`, `mime_type`, `disk`, `conversions_disk`, `size`, `manipulations`, `custom_properties`, `generated_conversions`, `responsive_images`, `order_column`, timestamps). A Laravel app can use the same table.
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

- **Find it:** while signed in as staff with `pages.view`, every public page shows a small floating **Open in admin** button (bottom right, no layout space). It opens the admin **show page** (`/admin/pages/{id}`); click **Edit** there. Visitors never see it, and it is never stored in the cached page.
- **Show page:** read-only, with both languages rendered as visitors see them and the feature image.
- **Edit page:** `/admin/pages/{id}/edit` has a text editor with a tab per language (title, search description, body HTML), the feature image (upload with alt text, replace, remove), and hide or publish. Reading needs `pages.view`; saving needs `pages.manage`.
- **Safe HTML:** the body is cleaned with the `ammonia` crate when saved, so scripts, event handlers and `javascript:` links never reach visitors. Headings, lists, links, images, tables and the like are kept.
- **All or nothing:** every language is checked before anything is saved, and the form shows again with what was typed if something is wrong. A language left completely empty is skipped; visitors who choose it see English.
- **Cache:** the public page is cached for five minutes; saving, uploading, removing, hiding or publishing clears it at once (`forget_cache`).

## The media library page

`/admin/media` (needs `media.view`) is a **table of every file of every model**, not a place to upload loose files. That matches Laravel's media library, where a file always belongs to a model.

| Column | What it shows |
|---|---|
| Preview | A thumbnail. **Click it** to open the image in a Magnific Popup lightbox; the images on the page form one gallery you can step through. PDFs open in a new tab. |
| Name | The display name and stored file name |
| Entity | The model that owns the file, for example **Page #3**. **Click it** to open that record's edit screen, where you change the file. |
| Collection, Type, Size | `feature_image`, `image/png`, 35 KB |
| Access | Public or private |
| Added | Date |

Search by name, filter by collection and type, 24 rows per page. jQuery and Magnific Popup are vendored in `assets/static/vendor/` and load only on this page.

### Direct upload (files without a record)

Every row has a `model_type` and `model_id`, the same as Spatie, so a loose file belongs to a built-in owner called **`library`** (id 0). The upload box at the top of `/admin/media` (needs `media.manage`) uses it:

- Drag and drop, or browse. Pick **several files**; each one is sent in its own request, so each is checked and reported separately and stays under Loco's 10 MB request limit.
- PNG, JPG, WebP, GIF or PDF, up to 10 MB each. The content is checked, not the name.
- Library files are **public**. Use **Copy link** to paste the address into a page's text.
- **Delete** exists only for library files. A file that belongs to a record (like a page's feature image) is changed on that record, and the library refuses to delete it.
- Without JavaScript the same form uploads one file at a time.

To make a model show a link in the Entity column, add it to `owner_link()` in `modules/media/src/controllers/mod.rs`.

## Receiving uploads in your own handler

```rust
let form = module_media::read_form(multipart, "image").await;   // file + the other text fields
let alt = form.fields.get("alt").cloned().unwrap_or_default();
let file = form.into_file()?;                                    // Err(Empty) or Err(TooLarge) explains why
```

Loco limits a request body to 10 MB (set by `limit_payload` in `config/*.yaml`). A bigger upload breaks the stream, and `into_file` reports that as `TooLarge` so the visitor sees "too large", not "nothing chosen".

<!-- foot:start -->
---
[Previous: 16 The admin dashboard (`admin-layout` module)](16-admin-dashboard.md) | [Index](00-index.md)
<!-- foot:end -->


## Sharing the database with a Laravel (Spatie) app

The media module uses Spatie's table and file layout, so you can move a Laravel project over without converting its media.

- **File layout:** a file lives at `{id}/{file_name}` on the storage disk (with `settings.media.prefix` in front when set), the same as Spatie's `DefaultPathGenerator`. Point Loco's `storage` at the same bucket and existing files are found where Laravel left them.
- **Owners:** `Owner::new("enrollment", 77)` matches rows whose `model_type` is `enrollment` and `model_id` is `77`. Use the same alias (or full class name) Laravel stores. `model_id` is text, so uuid keys are fine; only numeric keys can be read through `owner_id()`.
- **Import:** `cargo run -- task media:import database:bimafy_sep24` copies another database's `media` table on the same server into this one, keeping ids. It can be run again; rows already present are skipped. If both apps use one database there is nothing to import.
- **Public or private:** an imported file is private (only its owner and staff with `media.view` can open it) until it has `"public": true` in `custom_properties`. Collections declared `.public()` set that on new files. The `disk` column keeps Laravel's name (for example `s3-media`); new rows get `settings.media.disk`.
- **CDN:** set `settings.media.disk_urls.<disk>` and public files at `/media/{id}/{file}` redirect to `{url}/{path}`, so their bytes do not pass through this app.
- **Not covered:** image conversions and responsive images. Their columns are kept and written as `[]`.

```yaml
settings:
  media:
    disk: s3-media
    prefix: ""
    disk_urls:
      s3-media: https://cdn.example.com
```
