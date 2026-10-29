use axum_test::multipart::{MultipartForm, Part};
use loco_rs::{app::AppContext, testing::prelude::*, TestServer};
use module_media::{
    CollectionDef, HasMedia, InteractsWithMedia, MediaError, Owner, Upload, IMAGES,
};
use rust_starter::{
    app::App,
    seeders::demo_user::{self, DEMO_EMAIL, DEMO_PASSWORD},
};
use serial_test::serial;
use std::path::Path;

const ORIGIN: &str = "http://localhost";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01";
const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, b'J', b'F', b'I', b'F'];

fn png(name: &str) -> Upload {
    Upload::new(name, PNG)
}

fn covers() -> Owner {
    Owner::new("post", 1).with_collections(vec![
        CollectionDef::single("cover")
            .accepts_mime_types(IMAGES)
            .max_bytes(64),
        CollectionDef::multiple("gallery")
            .only_keep_latest(2)
            .accepts_mime_types(IMAGES),
        CollectionDef::multiple("attachments"),
    ])
}

async fn exists(ctx: &AppContext, path: &str) -> bool {
    ctx.storage.exists(Path::new(path)).await.unwrap()
}

#[tokio::test]
#[serial]
async fn single_file_collection_replaces_the_old_file() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    let owner = covers();

    let first = owner
        .add_media(&ctx, png("one.png"))
        .to_media_collection("cover")
        .await
        .unwrap();
    let second = owner
        .add_media(&ctx, png("two.png"))
        .using_name("Second cover")
        .to_media_collection("cover")
        .await
        .unwrap();

    let all = owner.get_media(&ctx, "cover").await.unwrap();
    assert_eq!(
        all.len(),
        1,
        "a single-file collection keeps exactly one file"
    );
    assert_eq!(all[0].id, second.id);
    assert_eq!(all[0].name, "Second cover");
    assert!(
        !exists(&ctx, &first.storage_path()).await,
        "the replaced file is removed from storage"
    );
    assert!(exists(&ctx, &second.storage_path()).await);
}

#[tokio::test]
#[serial]
async fn multiple_collection_keeps_many_and_can_keep_only_the_latest_n() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    let owner = covers();

    for index in 0..4 {
        owner
            .add_media(&ctx, png(&format!("file-{index}.png")))
            .to_media_collection("attachments")
            .await
            .unwrap();
    }
    // "attachments" accepts any recognised type, so all four stay.
    assert_eq!(owner.get_media(&ctx, "attachments").await.unwrap().len(), 4);

    let mut gallery = Vec::new();
    for index in 0..3 {
        gallery.push(
            owner
                .add_media(&ctx, png(&format!("g{index}.png")))
                .to_media_collection("gallery")
                .await
                .unwrap(),
        );
    }
    let kept = owner.get_media(&ctx, "gallery").await.unwrap();
    assert_eq!(kept.len(), 2, "only_keep_latest(2)");
    assert_eq!(
        kept.iter().map(|m| m.id).collect::<Vec<_>>(),
        [gallery[1].id, gallery[2].id],
        "the oldest one went"
    );
    assert!(!exists(&ctx, &gallery[0].storage_path()).await);
    assert!(
        kept[0].order_column < kept[1].order_column,
        "files keep their order"
    );
}

#[tokio::test]
#[serial]
async fn uploads_are_validated_by_content_size_and_collection_rules() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    let owner = covers();

    let fake = owner
        .add_media(
            &ctx,
            Upload::new("evil.png", &b"<?php system($_GET['c']); ?>"[..]),
        )
        .to_media_collection("gallery")
        .await;
    assert_eq!(
        fake.unwrap_err(),
        MediaError::UnsupportedType,
        "the name says png, the content says no"
    );

    let pdf = owner
        .add_media(&ctx, Upload::new("doc.pdf", &b"%PDF-1.7 test"[..]))
        .to_media_collection("gallery")
        .await;
    assert_eq!(
        pdf.unwrap_err(),
        MediaError::UnsupportedType,
        "gallery only accepts images"
    );
    assert!(owner
        .add_media(&ctx, Upload::new("doc.pdf", &b"%PDF-1.7 test"[..]))
        .to_media_collection("attachments")
        .await
        .is_ok());

    let empty = owner
        .add_media(&ctx, Upload::new("x.png", Vec::new()))
        .to_media_collection("gallery")
        .await;
    assert_eq!(empty.unwrap_err(), MediaError::Empty);

    let mut big = PNG.to_vec();
    big.resize(65, 0);
    let too_large = owner
        .add_media(&ctx, Upload::new("big.png", big))
        .to_media_collection("cover")
        .await;
    assert_eq!(
        too_large.unwrap_err(),
        MediaError::TooLarge { max_bytes: 64 }
    );

    assert!(
        owner.get_media(&ctx, "cover").await.unwrap().is_empty(),
        "rejected files leave nothing behind"
    );
}

#[tokio::test]
#[serial]
async fn stored_names_follow_the_content_and_are_safe() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    let owner = covers();

    let media = owner
        .add_media(&ctx, Upload::new("../../My Photo (1).EXE", JPEG))
        .with_custom_properties(serde_json::json!({ "alt": "A photo" }))
        .to_media_collection("gallery")
        .await
        .unwrap();
    assert_eq!(
        media.file_name, "My-Photo-1.jpg",
        "extension comes from the content, not the client"
    );
    assert_eq!(media.mime_type, "image/jpeg");
    assert_eq!(media.name, "My Photo (1)");
    assert_eq!(
        media.custom_property("alt"),
        Some(serde_json::json!("A photo"))
    );
    assert!(media.url().starts_with("/media/") && media.url().ends_with("/My-Photo-1.jpg"));
}

/// Any model can own files by implementing `HasMedia`.
struct Post {
    id: i64,
}

impl HasMedia for Post {
    fn media_type(&self) -> String {
        "post".to_string()
    }
    fn media_id(&self) -> i64 {
        self.id
    }
    fn media_collections(&self) -> Vec<CollectionDef> {
        vec![
            CollectionDef::single("cover").accepts_mime_types(IMAGES),
            CollectionDef::multiple("gallery").accepts_mime_types(IMAGES),
        ]
    }
}

#[tokio::test]
#[serial]
async fn a_model_uses_media_through_the_trait() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    let post = Post { id: 7 };
    let other = Post { id: 8 };

    assert!(!post.has_media(&ctx, "cover").await.unwrap());
    post.add_media(&ctx, png("cover.png"))
        .to_media_collection("cover")
        .await
        .unwrap();
    post.add_media(&ctx, png("a.png"))
        .to_media_collection("gallery")
        .await
        .unwrap();
    let b = post
        .add_media(&ctx, png("b.png"))
        .to_media_collection("gallery")
        .await
        .unwrap();

    assert!(post.has_media(&ctx, "cover").await.unwrap());
    assert!(post
        .get_first_media_url(&ctx, "cover")
        .await
        .unwrap()
        .unwrap()
        .starts_with("/media/"));
    assert_eq!(
        post.get_last_media(&ctx, "gallery")
            .await
            .unwrap()
            .unwrap()
            .id,
        b.id
    );
    assert!(
        !other.has_media(&ctx, "cover").await.unwrap(),
        "files belong to one model only"
    );

    assert!(
        !other.delete_media(&ctx, b.id).await.unwrap(),
        "another model cannot delete it"
    );
    assert!(post.delete_media(&ctx, b.id).await.unwrap());
    assert_eq!(
        post.clear_media_collection(&ctx, "gallery").await.unwrap(),
        1
    );
    assert_eq!(
        post.clear_media_collection(&ctx, "gallery").await.unwrap(),
        0
    );
}

// ---------- HTTP: accounts and the admin library ----------

async fn login(server: &TestServer, email: &str, password: &str) -> String {
    let response = server
        .post("/login")
        .add_header("host", "localhost")
        .add_header("origin", ORIGIN)
        .form(&[("email", email), ("password", password)])
        .await;
    assert_eq!(response.status_code(), 303);
    response
        .header("set-cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

async fn user(ctx: &AppContext, email: &str, role: Option<&str>) {
    use rust_starter::models::users::{self, RegisterParams};
    use sea_orm::IntoActiveModel;
    module_role_permission::sync(&ctx.db).await.unwrap();
    let created = users::Model::create_with_password(
        &ctx.db,
        &RegisterParams {
            email: email.into(),
            password: "password123".into(),
            name: "Test Person".into(),
        },
    )
    .await
    .unwrap();
    let created = created.into_active_model().verified(&ctx.db).await.unwrap();
    if let Some(role) = role {
        module_role_permission::assign_role(&ctx.db, created.id, role)
            .await
            .unwrap();
    }
}

fn post(server: &TestServer, path: &str, cookie: &str) -> axum_test::TestRequest {
    server
        .post(path)
        .add_header("host", "localhost")
        .add_header("origin", ORIGIN)
        .add_header("cookie", cookie.to_string())
}

fn logo_form(bytes: Vec<u8>, name: &str) -> MultipartForm {
    MultipartForm::new().add_part(
        "logo",
        Part::bytes(bytes).file_name(name).mime_type("image/png"),
    )
}

#[tokio::test]
#[serial]
async fn account_pages_need_login_and_share_one_sub_layout() {
    request::<App, _, _>(|server, ctx| async move {
        for path in [
            "/account",
            "/account/profile",
            "/account/password",
            "/account/logo",
        ] {
            let guest = server.get(path).await;
            assert_eq!(guest.status_code(), 303, "{path}");
            assert!(guest
                .header("location")
                .to_str()
                .unwrap()
                .starts_with("/login?next="));
        }

        demo_user::run(&ctx.db).await.unwrap();
        let cookie = login(&server, DEMO_EMAIL, DEMO_PASSWORD).await;
        for (path, nav) in [
            ("/account/profile", "profile"),
            ("/account/password", "password"),
            ("/account/logo", "logo"),
        ] {
            let html = server
                .get(path)
                .add_header("cookie", cookie.clone())
                .await
                .text();
            assert!(
                html.contains(r#"data-testid="account-logout""#),
                "{path} has the logout button"
            );
            assert!(
                html.contains(r#"data-testid="account-nav-profile""#)
                    && html.contains(r#"data-testid="account-nav-password""#)
                    && html.contains(r#"data-testid="account-nav-logo""#)
            );
            assert!(
                html.contains(&format!(
                    r#"data-testid="account-nav-{nav}" aria-current="page""#
                )),
                "{path} marks its own link"
            );
        }
        let html = server
            .get("/account/profile")
            .add_header("cookie", cookie)
            .await
            .text();
        assert!(
            html.contains("Demo User") && html.contains(DEMO_EMAIL) && html.contains("Developer")
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn profile_name_can_be_changed_and_is_validated() {
    request::<App, _, _>(|server, ctx| async move {
        user(&ctx, "pat@example.com", None).await;
        let cookie = login(&server, "pat@example.com", "password123").await;

        let bad = post(&server, "/account/profile", &cookie)
            .form(&[("name", "A")])
            .await;
        assert_eq!(bad.status_code(), 422);
        assert!(bad.text().contains("Enter a name of 2 to 80 characters."));

        let ok = post(&server, "/account/profile", &cookie)
            .form(&[("name", "  Pat Morgan ")])
            .await;
        assert_eq!(ok.status_code(), 303);
        assert_eq!(
            ok.header("location"),
            "/account/profile?status=profile-saved"
        );
        let page = server
            .get("/account/profile?status=profile-saved")
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(page.contains("Pat Morgan") && page.contains("Your profile was updated."));

        let foreign = server
            .post("/account/profile")
            .add_header("host", "localhost")
            .add_header("origin", "https://evil.example")
            .add_header("cookie", cookie)
            .form(&[("name", "Hacked")])
            .await;
        assert_eq!(foreign.status_code(), 403);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn changing_the_password_signs_out_other_devices() {
    request::<App, _, _>(|server, ctx| async move {
        user(&ctx, "sam@example.com", None).await;
        let this_device = login(&server, "sam@example.com", "password123").await;
        let other_device = login(&server, "sam@example.com", "password123").await;

        for (current, new, confirm, message) in [
            (
                "wrong-password",
                "newpassword1",
                "newpassword1",
                "Your current password is incorrect.",
            ),
            (
                "password123",
                "short",
                "short",
                "The new password must be at least 8 characters.",
            ),
            (
                "password123",
                "newpassword1",
                "different1234",
                "The new passwords do not match.",
            ),
        ] {
            let response = post(&server, "/account/password", &this_device)
                .form(&[
                    ("current_password", current),
                    ("password", new),
                    ("password_confirmation", confirm),
                ])
                .await;
            assert_eq!(response.status_code(), 422);
            assert!(response.text().contains(message), "expected `{message}`");
        }

        let ok = post(&server, "/account/password", &this_device)
            .form(&[
                ("current_password", "password123"),
                ("password", "newpassword1"),
                ("password_confirmation", "newpassword1"),
            ])
            .await;
        assert_eq!(ok.status_code(), 303);

        assert_eq!(
            server
                .get("/account/profile")
                .add_header("cookie", this_device)
                .await
                .status_code(),
            200,
            "this device stays signed in"
        );
        assert_eq!(
            server
                .get("/account/profile")
                .add_header("cookie", other_device)
                .await
                .status_code(),
            303,
            "the other device is signed out"
        );

        let old = server
            .post("/login")
            .add_header("host", "localhost")
            .add_header("origin", ORIGIN)
            .form(&[("email", "sam@example.com"), ("password", "password123")])
            .await;
        assert_eq!(old.status_code(), 401);
        login(&server, "sam@example.com", "newpassword1").await;
    })
    .await;
}

#[tokio::test]
#[serial]
async fn logo_upload_replaces_shows_in_the_header_and_stays_private() {
    request::<App, _, _>(|server, ctx| async move {
        user(&ctx, "lee@example.com", None).await;
        user(&ctx, "other@example.com", None).await;
        let cookie = login(&server, "lee@example.com", "password123").await;
        let other = login(&server, "other@example.com", "password123").await;

        assert!(server
            .get("/account/logo")
            .add_header("cookie", cookie.clone())
            .await
            .text()
            .contains(r#"data-testid="logo-none""#));

        let ok = post(&server, "/account/logo", &cookie)
            .multipart(logo_form(PNG.to_vec(), "my logo.png"))
            .await;
        assert_eq!(ok.status_code(), 303, "{}", ok.text());
        let page = server
            .get("/account/logo?status=logo-saved")
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(
            page.contains(r#"data-testid="logo-preview""#)
                && page.contains("Your logo was updated.")
        );
        let url = page
            .split(r#"data-testid="logo-preview""#)
            .next()
            .unwrap()
            .rsplit(r#"src=""#)
            .next()
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_string();
        assert!(
            url.starts_with("/media/") && url.ends_with("/my-logo.png"),
            "url was {url}"
        );

        let home = server
            .get("/")
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(
            home.contains(&format!(r#"src="{url}""#)),
            "the header avatar shows the logo"
        );

        let own = server.get(&url).add_header("cookie", cookie.clone()).await;
        own.assert_status_ok();
        assert_eq!(own.header("content-type"), "image/png");
        assert_eq!(own.header("x-content-type-options"), "nosniff");
        assert_eq!(
            server.get(&url).await.status_code(),
            404,
            "guests cannot fetch a private logo"
        );
        assert_eq!(
            server
                .get(&url)
                .add_header("cookie", other)
                .await
                .status_code(),
            404,
            "neither can another user"
        );

        // A second upload replaces the first (single-file collection).
        post(&server, "/account/logo", &cookie)
            .multipart(logo_form(JPEG.to_vec(), "new.jpg"))
            .await;
        assert_eq!(
            server
                .get(&url)
                .add_header("cookie", cookie.clone())
                .await
                .status_code(),
            404,
            "the old file is gone"
        );

        let removed = post(&server, "/account/logo/remove", &cookie).await;
        assert_eq!(
            removed.header("location"),
            "/account/logo?status=logo-removed"
        );
        assert!(server
            .get("/account/logo")
            .add_header("cookie", cookie)
            .await
            .text()
            .contains(r#"data-testid="logo-none""#));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn logo_upload_rejects_bad_files() {
    request::<App, _, _>(|server, ctx| async move {
        user(&ctx, "lee@example.com", None).await;
        let cookie = login(&server, "lee@example.com", "password123").await;

        let text = post(&server, "/account/logo", &cookie)
            .multipart(logo_form(b"just text".to_vec(), "logo.png"))
            .await;
        assert_eq!(text.status_code(), 422);
        assert!(text.text().contains("Upload a PNG, JPG or WebP image."));

        let mut big = PNG.to_vec();
        big.resize(1024 * 1024 + 1, 0);
        let large = post(&server, "/account/logo", &cookie)
            .multipart(logo_form(big, "big.png"))
            .await;
        assert_eq!(large.status_code(), 422);
        assert!(large.text().contains("The image is larger than 1 MB."));

        let pdf = post(&server, "/account/logo", &cookie)
            .multipart(
                MultipartForm::new()
                    .add_part("logo", Part::bytes(b"%PDF-1.7".to_vec()).file_name("a.pdf")),
            )
            .await;
        assert_eq!(pdf.status_code(), 422);
    })
    .await;
}

// ---------- pages own a feature image through the media library ----------

async fn page_id(ctx: &AppContext, slug: &str) -> i64 {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    module_pages::pages::Entity::find()
        .filter(module_pages::pages::Column::Slug.eq(slug))
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap()
        .id
}

fn image_form(bytes: Vec<u8>, name: &str, alt: &str) -> MultipartForm {
    MultipartForm::new()
        .add_part(
            "image",
            Part::bytes(bytes).file_name(name).mime_type("image/png"),
        )
        .add_text("alt", alt)
}

#[tokio::test]
#[serial]
async fn a_page_feature_image_is_uploaded_in_the_admin_and_shown_on_the_public_page() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        let id = page_id(&ctx, "about").await;
        let dev = login(&server, DEMO_EMAIL, DEMO_PASSWORD).await;

        // Warm the page cache first, so this also proves the cache is cleared on change.
        let before = server.get("/pages/about").await.text();
        assert!(!before.contains(r#"data-testid="feature-image""#));

        let edit = server
            .get(&format!("/admin/pages/{id}/edit"))
            .add_header("cookie", dev.clone())
            .await
            .text();
        assert!(
            edit.contains(r#"data-testid="feature-image-none""#)
                && edit.contains(r#"data-testid="feature-image-form""#)
        );

        let upload = post(&server, &format!("/admin/pages/{id}/feature-image"), &dev)
            .multipart(image_form(
                PNG.to_vec(),
                "Hero Photo.png",
                "A sunrise over the river",
            ))
            .await;
        assert_eq!(upload.status_code(), 303);
        assert_eq!(
            upload.header("location"),
            format!("/admin/pages/{id}/edit?saved=image")
        );

        let public = server.get("/pages/about").await.text();
        assert!(
            public.contains(r#"data-testid="feature-image""#),
            "the public page shows the image right away"
        );
        assert!(public.contains(r#"alt="A sunrise over the river""#));
        assert!(
            public.contains(r#"<meta property="og:image" content="http"#)
                && public.contains("/media/"),
            "social sharing gets an absolute image address"
        );
        let url = public
            .split(r#"data-testid="feature-image""#)
            .next()
            .unwrap()
            .rsplit(r#"src=""#)
            .next()
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_string();
        assert!(
            url.starts_with("/media/") && url.ends_with("/Hero-Photo.png"),
            "url was {url}"
        );
        let image = server.get(&url).await;
        image.assert_status_ok();
        assert_eq!(
            image.header("content-type"),
            "image/png",
            "a feature image is public: no login needed"
        );

        // The other language shows it too.
        assert!(server
            .get("/pages/about?lang=bn")
            .await
            .text()
            .contains(r#"data-testid="feature-image""#));

        // Single-file collection: a second upload replaces the first.
        post(&server, &format!("/admin/pages/{id}/feature-image"), &dev)
            .multipart(image_form(JPEG.to_vec(), "second.jpg", ""))
            .await;
        assert_eq!(
            server.get(&url).await.status_code(),
            404,
            "the old file is gone"
        );
        assert!(server
            .get("/pages/about")
            .await
            .text()
            .contains("/second.jpg"));

        // Removing it clears the page again.
        let removed = post(
            &server,
            &format!("/admin/pages/{id}/feature-image/remove"),
            &dev,
        )
        .await;
        assert_eq!(
            removed.header("location"),
            format!("/admin/pages/{id}/edit?saved=removed")
        );
        assert!(!server
            .get("/pages/about")
            .await
            .text()
            .contains(r#"data-testid="feature-image""#));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn feature_image_uploads_are_validated_and_need_pages_manage() {
    request::<App, _, _>(|server, ctx| async move {
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        user(&ctx, "market@example.com", Some("marketing")).await; // pages.manage
        user(&ctx, "biz@example.com", Some("business_development")).await; // pages.view only
        user(&ctx, "plain@example.com", None).await;
        let id = page_id(&ctx, "about").await;
        let marketer = login(&server, "market@example.com", "password123").await;
        let viewer = login(&server, "biz@example.com", "password123").await;
        let plain = login(&server, "plain@example.com", "password123").await;
        let path = format!("/admin/pages/{id}/feature-image");

        // Bad uploads from someone who may manage.
        let text = post(&server, &path, &marketer)
            .multipart(image_form(b"not an image".to_vec(), "x.png", ""))
            .await;
        assert_eq!(
            text.header("location"),
            format!("/admin/pages/{id}/edit?error=type")
        );
        let pdf = post(&server, &path, &marketer)
            .multipart(MultipartForm::new().add_part(
                "image",
                Part::bytes(b"%PDF-1.7".to_vec()).file_name("a.pdf"),
            ))
            .await;
        assert_eq!(
            pdf.header("location"),
            format!("/admin/pages/{id}/edit?error=type")
        );
        let mut big = PNG.to_vec();
        big.resize(2 * 1024 * 1024 + 1, 0);
        let large = post(&server, &path, &marketer)
            .multipart(image_form(big, "big.png", ""))
            .await;
        // Loco caps the whole request body at 2 MB, so the stream breaks; that is reported as too large.
        assert_eq!(
            large.header("location"),
            format!("/admin/pages/{id}/edit?error=size")
        );
        let none = post(&server, &path, &marketer)
            .multipart(MultipartForm::new().add_text("alt", "only text"))
            .await;
        assert_eq!(
            none.header("location"),
            format!("/admin/pages/{id}/edit?error=empty")
        );

        // Marketing can upload; the shown message follows the result.
        assert_eq!(
            post(&server, &path, &marketer)
                .multipart(image_form(PNG.to_vec(), "ok.png", "ok"))
                .await
                .status_code(),
            303
        );

        // A viewer sees the page but has no form and cannot post.
        let view = server
            .get(&format!("/admin/pages/{id}/edit"))
            .add_header("cookie", viewer.clone())
            .await;
        view.assert_status_ok();
        assert!(
            !view.text().contains(r#"data-testid="feature-image-form""#)
                && view
                    .text()
                    .contains(r#"data-testid="feature-image-preview""#)
        );
        assert_eq!(
            post(&server, &path, &viewer)
                .multipart(image_form(PNG.to_vec(), "no.png", ""))
                .await
                .status_code(),
            403
        );
        assert_eq!(
            post(&server, &format!("{path}/remove"), &viewer)
                .await
                .status_code(),
            403
        );

        // Someone with no role cannot even see the page.
        assert_eq!(
            server
                .get(&format!("/admin/pages/{id}/edit"))
                .add_header("cookie", plain.clone())
                .await
                .status_code(),
            403
        );
        assert_eq!(
            server
                .get("/admin/pages")
                .add_header("cookie", plain)
                .await
                .status_code(),
            403
        );

        // Forms from other sites are refused, and guests are sent to log in.
        let foreign = server
            .post(&path)
            .add_header("host", "localhost")
            .add_header("origin", "https://evil.example")
            .add_header("cookie", marketer)
            .multipart(image_form(PNG.to_vec(), "x.png", ""))
            .await;
        assert_eq!(foreign.status_code(), 403);
        assert_eq!(
            server
                .get(&format!("/admin/pages/{id}/edit"))
                .await
                .status_code(),
            303
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_page_can_be_hidden_and_published_again() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        let id = page_id(&ctx, "privacy-policy").await;
        let dev = login(&server, DEMO_EMAIL, DEMO_PASSWORD).await;

        assert_eq!(server.get("/pages/privacy-policy").await.status_code(), 200);
        let hide = post(&server, &format!("/admin/pages/{id}/published"), &dev)
            .form(&[("published", "0")])
            .await;
        assert_eq!(
            hide.header("location"),
            format!("/admin/pages/{id}/edit?saved=unpublished")
        );
        assert_eq!(
            server.get("/pages/privacy-policy").await.status_code(),
            404,
            "hidden pages disappear at once, cache included"
        );

        post(&server, &format!("/admin/pages/{id}/published"), &dev)
            .form(&[("published", "1")])
            .await;
        assert_eq!(server.get("/pages/privacy-policy").await.status_code(), 200);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn admin_page_list_and_media_library_show_the_feature_image() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        user(&ctx, "plain@example.com", None).await;
        let id = page_id(&ctx, "terms-of-service").await;
        let dev = login(&server, DEMO_EMAIL, DEMO_PASSWORD).await;
        post(&server, &format!("/admin/pages/{id}/feature-image"), &dev)
            .multipart(image_form(PNG.to_vec(), "Terms Banner.png", "Banner"))
            .await;

        let list = server
            .get("/admin/pages")
            .add_header("cookie", dev.clone())
            .await
            .text();
        assert!(
            list.contains("Terms of Service")
                && list.contains("About us")
                && list.contains("Privacy Policy")
        );
        assert!(
            list.contains(&format!(r#"href="/admin/pages/{id}/edit""#))
                && list.contains("/Terms-Banner.png"),
            "the list shows each page's image"
        );

        let library = server
            .get("/admin/media")
            .add_header("cookie", dev.clone())
            .await
            .text();
        assert!(library.contains(r#"data-testid="media-table""#) && library.contains("1 files"));
        assert!(
            library.contains(r#"class="media-popup"#)
                && library.contains(r#"data-testid="media-thumb""#),
            "thumbnails are lightbox links"
        );
        assert!(
            library.contains("/static/vendor/magnific-popup/jquery.magnific-popup.min.js")
                && library.contains("magnificPopup("),
            "Magnific Popup is loaded and started"
        );
        assert!(
            library.contains(r#"data-testid="media-entity""#),
            "each row names its entity"
        );
        for asset in [
            "/static/vendor/jquery/jquery.min.js",
            "/static/vendor/magnific-popup/jquery.magnific-popup.min.js",
            "/static/vendor/magnific-popup/magnific-popup.min.css",
        ] {
            assert_eq!(
                server.get(asset).await.status_code(),
                200,
                "{asset} is served"
            );
        }
        assert!(
            library.contains("feature_image")
                && library.contains("Page #")
                && library.contains(&format!(r#"href="/admin/pages/{id}/edit""#)),
            "the library links to the owner"
        );

        // Filters.
        let found = server
            .get("/admin/media?q=terms&kind=image&collection=feature_image")
            .add_header("cookie", dev.clone())
            .await
            .text();
        assert!(found.contains("1 files"));
        let missing = server
            .get("/admin/media?q=nothing-matches")
            .add_header("cookie", dev.clone())
            .await
            .text();
        assert!(missing.contains("0 files") && missing.contains("No files found."));
        let pdfs = server
            .get("/admin/media?kind=pdf")
            .add_header("cookie", dev)
            .await
            .text();
        assert!(pdfs.contains("0 files"));

        let plain = login(&server, "plain@example.com", "password123").await;
        assert_eq!(
            server
                .get("/admin/media")
                .add_header("cookie", plain)
                .await
                .status_code(),
            403
        );
        assert_eq!(server.get("/admin/media").await.status_code(), 303);
    })
    .await;
}

// ---------- direct upload in the media library (files without a record) ----------

fn file_form(bytes: Vec<u8>, name: &str, mime: &str) -> MultipartForm {
    MultipartForm::new().add_part("file", Part::bytes(bytes).file_name(name).mime_type(mime))
}

fn first_library_url(html: &str) -> String {
    html.split(r#"data-copy=""#)
        .nth(1)
        .expect("a copy-link button")
        .split('"')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
#[serial]
async fn the_media_library_accepts_direct_uploads_without_any_record() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        let dev = login(&server, DEMO_EMAIL, DEMO_PASSWORD).await;

        let page = server
            .get("/admin/media")
            .add_header("cookie", dev.clone())
            .await
            .text();
        assert!(
            page.contains(r#"data-testid="media-upload""#) && page.contains(r#"id="dropzone""#),
            "the upload box is on the page"
        );
        assert!(page.contains("Drag and drop files here"));

        let png = post(&server, "/admin/media/upload", &dev)
            .multipart(file_form(PNG.to_vec(), "Team Photo.png", "image/png"))
            .await;
        assert_eq!(png.status_code(), 303);
        assert_eq!(png.header("location"), "/admin/media?uploaded=1");
        post(&server, "/admin/media/upload", &dev)
            .multipart(file_form(
                b"%PDF-1.7 brochure".to_vec(),
                "brochure.pdf",
                "application/pdf",
            ))
            .await;

        let library = server
            .get("/admin/media?uploaded=1")
            .add_header("cookie", dev.clone())
            .await
            .text();
        assert!(
            library.contains("1 file uploaded."),
            "the message says what happened"
        );
        assert!(library.contains("2 files"), "both files are listed");
        assert!(
            library.contains("Media library") && library.contains("loose file, no record"),
            "the entity column explains there is no record"
        );
        assert!(library.contains("Team Photo") && library.contains("brochure"));
        assert!(
            library.contains(r#"data-testid="media-copy""#)
                && library.contains(r#"data-testid="media-delete""#)
        );
        assert!(
            library.contains(r#"class="media-popup"#),
            "uploaded images open in the lightbox too"
        );

        // Library files are public, so the copied address works for anyone.
        let url = first_library_url(&library);
        assert!(url.starts_with("/media/"));
        server.get(&url).await.assert_status_ok();
    })
    .await;
}

#[tokio::test]
#[serial]
async fn direct_uploads_are_validated_one_file_at_a_time() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        let dev = login(&server, DEMO_EMAIL, DEMO_PASSWORD).await;
        let upload =
            |form: MultipartForm| post(&server, "/admin/media/upload", &dev).multipart(form);

        let bad = upload(file_form(
            b"MZ binary, not an image".to_vec(),
            "virus.png",
            "image/png",
        ))
        .await;
        assert_eq!(
            bad.header("location"),
            "/admin/media?failed=1&reason=type",
            "the content decides, not the name"
        );
        let svg = upload(file_form(
            b"<svg onload=alert(1)></svg>".to_vec(),
            "x.svg",
            "image/svg+xml",
        ))
        .await;
        assert_eq!(svg.header("location"), "/admin/media?failed=1&reason=type");
        let none = upload(MultipartForm::new().add_text("note", "no file")).await;
        assert_eq!(
            none.header("location"),
            "/admin/media?failed=1&reason=empty"
        );
        let mut big = PNG.to_vec();
        big.resize(2 * 1024 * 1024 + 1, 0);
        let large = upload(file_form(big, "huge.png", "image/png")).await;
        assert_eq!(
            large.header("location"),
            "/admin/media?failed=1&reason=size"
        );

        // The page explains the failure in words.
        let page = server
            .get("/admin/media?failed=1&reason=size")
            .add_header("cookie", dev.clone())
            .await
            .text();
        assert!(
            page.contains("1 file could not be uploaded.") && page.contains("larger than 2 MB")
        );
        let page = server
            .get("/admin/media?uploaded=2&failed=1&reason=type")
            .add_header("cookie", dev.clone())
            .await
            .text();
        assert!(
            page.contains("2 uploaded, 1 failed.")
                && page.contains("Only PNG, JPG, WebP, GIF and PDF")
        );

        assert!(
            server
                .get("/admin/media")
                .add_header("cookie", dev)
                .await
                .text()
                .contains("0 files"),
            "nothing bad was stored"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn direct_upload_and_delete_need_media_manage() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        user(&ctx, "market@example.com", Some("marketing")).await; // media.view only
        user(&ctx, "plain@example.com", None).await;
        let dev = login(&server, DEMO_EMAIL, DEMO_PASSWORD).await;
        let marketer = login(&server, "market@example.com", "password123").await;
        let plain = login(&server, "plain@example.com", "password123").await;
        post(&server, "/admin/media/upload", &dev)
            .multipart(file_form(PNG.to_vec(), "keep.png", "image/png"))
            .await;
        let id = {
            use sea_orm::EntityTrait;
            module_media::media_files::Entity::find()
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap()
                .id
        };

        // View-only staff can look but get no upload box, no delete button, and their posts are refused.
        let viewer = server
            .get("/admin/media")
            .add_header("cookie", marketer.clone())
            .await;
        viewer.assert_status_ok();
        assert!(
            !viewer.text().contains(r#"data-testid="media-upload""#)
                && !viewer.text().contains(r#"data-testid="media-delete""#)
        );
        assert_eq!(
            post(&server, "/admin/media/upload", &marketer)
                .multipart(file_form(PNG.to_vec(), "no.png", "image/png"))
                .await
                .status_code(),
            403
        );
        assert_eq!(
            post(&server, &format!("/admin/media/{id}/delete"), &marketer)
                .await
                .status_code(),
            403
        );

        // No role: no access at all. Guests are sent to log in.
        assert_eq!(
            server
                .get("/admin/media")
                .add_header("cookie", plain.clone())
                .await
                .status_code(),
            403
        );
        assert_eq!(
            post(&server, "/admin/media/upload", &plain)
                .multipart(file_form(PNG.to_vec(), "no.png", "image/png"))
                .await
                .status_code(),
            403
        );
        assert_eq!(server.get("/admin/media").await.status_code(), 303);

        // Other sites cannot post uploads or deletes on a developer's behalf.
        let foreign = server
            .post("/admin/media/upload")
            .add_header("host", "localhost")
            .add_header("origin", "https://evil.example")
            .add_header("cookie", dev.clone())
            .multipart(file_form(PNG.to_vec(), "x.png", "image/png"))
            .await;
        assert_eq!(foreign.status_code(), 403);
        let foreign = server
            .post(&format!("/admin/media/{id}/delete"))
            .add_header("host", "localhost")
            .add_header("origin", "https://evil.example")
            .add_header("cookie", dev.clone())
            .await;
        assert_eq!(foreign.status_code(), 403);

        // The developer can delete a loose file.
        let url = first_library_url(
            &server
                .get("/admin/media")
                .add_header("cookie", dev.clone())
                .await
                .text(),
        );
        server.get(&url).await.assert_status_ok();
        let deleted = post(&server, &format!("/admin/media/{id}/delete"), &dev).await;
        assert_eq!(deleted.header("location"), "/admin/media?deleted=1");
        assert_eq!(
            server.get(&url).await.status_code(),
            404,
            "the file is gone from storage too"
        );
        assert!(server
            .get("/admin/media?deleted=1")
            .add_header("cookie", dev)
            .await
            .text()
            .contains("File deleted."));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn the_library_never_deletes_files_that_belong_to_a_record() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        let id = page_id(&ctx, "about").await;
        let dev = login(&server, DEMO_EMAIL, DEMO_PASSWORD).await;
        post(&server, &format!("/admin/pages/{id}/feature-image"), &dev)
            .multipart(image_form(PNG.to_vec(), "owned.png", ""))
            .await;

        let owned = {
            use sea_orm::EntityTrait;
            module_media::media_files::Entity::find()
                .one(&ctx.db)
                .await
                .unwrap()
                .unwrap()
        };
        assert_eq!(owned.model_type, "page");

        let library = server
            .get("/admin/media")
            .add_header("cookie", dev.clone())
            .await
            .text();
        assert!(
            library.contains(&format!(r#"href="/admin/pages/{id}/edit""#)),
            "its entity is a link to the page"
        );
        assert!(
            !library.contains(r#"data-testid="media-delete""#),
            "no delete button for a file that belongs to a page"
        );

        let attempt = post(&server, &format!("/admin/media/{}/delete", owned.id), &dev).await;
        assert_eq!(attempt.status_code(), 404);
        server
            .get("/pages/about")
            .await
            .text()
            .contains(r#"data-testid="feature-image""#)
            .then_some(())
            .expect("the page still shows its image");
    })
    .await;
}
