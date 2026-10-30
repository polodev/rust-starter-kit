use loco_rs::{app::AppContext, testing::prelude::*, TestServer};
use module_auth::users as auth_users;
use rust_starter_pack_helpers::*;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serial_test::serial;

mod rust_starter_pack_helpers {
    use super::*;
    use rust_starter::{
        app::App,
        models::users::{self, RegisterParams},
    };
    use sea_orm::IntoActiveModel;

    pub const ORIGIN: &str = "http://localhost";

    pub async fn login(server: &TestServer, email: &str, password: &str) -> String {
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

    pub async fn user(ctx: &AppContext, email: &str, role: Option<&str>) {
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

    pub fn post(server: &TestServer, path: &str, cookie: &str) -> axum_test::TestRequest {
        server
            .post(path)
            .add_header("host", "localhost")
            .add_header("origin", ORIGIN)
            .add_header("cookie", cookie.to_string())
    }

    pub fn _app() -> std::marker::PhantomData<App> {
        std::marker::PhantomData
    }
}

async fn found(ctx: &AppContext, email: &str) -> Option<auth_users::Model> {
    auth_users::Entity::find()
        .filter(auth_users::Column::Email.eq(email))
        .one(&ctx.db)
        .await
        .unwrap()
}

#[tokio::test]
#[serial]
async fn admin_edits_name_and_email_and_duplicates_are_refused() {
    request::<rust_starter::app::App, _, _>(|server, ctx| async move {
        user(&ctx, "boss@example.com", Some("developer")).await;
        let cookie = login(&server, "boss@example.com", "password123").await;
        let target = found(&ctx, "boss@example.com").await.unwrap();

        let list = server
            .get("/admin/users")
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(list.contains(&format!(r#"data-testid="show-user-{}""#, target.id)));
        assert!(list.contains(&format!(r#"data-testid="edit-user-{}""#, target.id)));

        let show = server
            .get(&format!("/admin/users/{}", target.id))
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(
            show.contains(r#"data-testid="edit-button""#)
                && !show.contains(r#"data-testid="profile-form""#)
        );
        let edit = server
            .get(&format!("/admin/users/{}/edit", target.id))
            .add_header("cookie", cookie.clone())
            .await;
        edit.assert_status_ok();
        assert!(edit.text().contains(r#"data-testid="profile-form""#));

        let saved = post(&server, &format!("/admin/users/{}", target.id), &cookie)
            .form(&[("name", "New Name"), ("email", "Renamed@Example.com")])
            .await;
        assert_eq!(saved.status_code(), 303);
        let after = auth_users::Entity::find_by_id(target.id)
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (after.name.as_str(), after.email.as_str()),
            ("New Name", "renamed@example.com")
        );

        for (name, email, error) in [
            ("X", "ok@example.com", "name"),
            ("Fine Name", "not-an-email", "email"),
        ] {
            let response = post(&server, &format!("/admin/users/{}", target.id), &cookie)
                .form(&[("name", name), ("email", email)])
                .await;
            assert!(response
                .header("location")
                .to_str()
                .unwrap()
                .ends_with(&format!("/edit?error={error}")));
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn generated_password_works_once_and_ends_old_logins() {
    request::<rust_starter::app::App, _, _>(|server, ctx| async move {
        user(&ctx, "boss@example.com", Some("developer")).await;
        let cookie = login(&server, "boss@example.com", "password123").await;
        // A second account, signed in on another device.
        let worker = {
            use rust_starter::models::users::{self, RegisterParams};
            use sea_orm::IntoActiveModel;
            let created = users::Model::create_with_password(
                &ctx.db,
                &RegisterParams {
                    email: "worker@example.com".into(),
                    password: "password123".into(),
                    name: "Worker".into(),
                },
            )
            .await
            .unwrap();
            created.into_active_model().verified(&ctx.db).await.unwrap()
        };
        let worker_cookie = login(&server, "worker@example.com", "password123").await;

        let response = post(
            &server,
            &format!("/admin/users/{}/password", worker.id),
            &cookie,
        )
        .await;
        response.assert_status_ok();
        assert_eq!(
            response.header("cache-control").to_str().unwrap(),
            "no-store"
        );
        let text = response.text();
        let marker = r#"data-testid="new-password">"#;
        let start = text.find(marker).unwrap() + marker.len();
        let password = &text[start..start + 16];
        assert!(password.bytes().all(|b| b.is_ascii_alphanumeric()));

        login(&server, "worker@example.com", password).await;
        let old = server
            .post("/login")
            .add_header("host", "localhost")
            .add_header("origin", ORIGIN)
            .form(&[("email", "worker@example.com"), ("password", "password123")])
            .await;
        assert_eq!(old.status_code(), 401);
        // The old device's session is gone.
        let home = server
            .get("/")
            .add_header("cookie", worker_cookie)
            .await
            .text();
        assert!(home.contains(r#"data-testid="login-link""#));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn people_without_manage_can_view_but_not_change() {
    request::<rust_starter::app::App, _, _>(|server, ctx| async move {
        user(&ctx, "viewer@example.com", Some("new_employee")).await;
        let cookie = login(&server, "viewer@example.com", "password123").await;
        let me = found(&ctx, "viewer@example.com").await.unwrap();
        let page = server
            .get(&format!("/admin/users/{}", me.id))
            .add_header("cookie", cookie.clone())
            .await;
        if page.status_code() == 200 {
            assert!(!page.text().contains(r#"data-testid="edit-button""#));
        }
        let edit = server
            .get(&format!("/admin/users/{}/edit", me.id))
            .add_header("cookie", cookie.clone())
            .await;
        assert_eq!(edit.status_code(), 403);
        for path in [
            format!("/admin/users/{}", me.id),
            format!("/admin/users/{}/password", me.id),
        ] {
            let response = post(&server, &path, &cookie)
                .form(&[("name", "Hacked"), ("email", "h@example.com")])
                .await;
            assert_eq!(response.status_code(), 403, "{path}");
        }
        assert_eq!(
            found(&ctx, "viewer@example.com").await.unwrap().name,
            "Test Person"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn profile_image_is_one_file_that_can_be_replaced_and_removed() {
    use axum_test::multipart::{MultipartForm, Part};
    request::<rust_starter::app::App, _, _>(|server, ctx| async move {
        user(&ctx, "boss@example.com", Some("developer")).await;
        let cookie = login(&server, "boss@example.com", "password123").await;
        let me = found(&ctx, "boss@example.com").await.unwrap();
        let png: Vec<u8> = vec![
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52,
            0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 0x1f, 0x15, 0xc4, 0x89, 0, 0, 0, 0x0d, 0x49,
            0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xff, 0xff, 0x3f, 0, 5, 0xfe, 2, 0xfe, 0xdc,
            0xcc, 0x59, 0xe7, 0, 0, 0, 0, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        let upload = |bytes: Vec<u8>, name: &'static str| {
            let form = MultipartForm::new().add_part(
                "image",
                Part::bytes(bytes).file_name(name).mime_type("image/png"),
            );
            post(&server, &format!("/admin/users/{}/image", me.id), &cookie).multipart(form)
        };

        upload(png.clone(), "a.png").await;
        upload(png.clone(), "b.png").await;
        let edit = server
            .get(&format!("/admin/users/{}/edit", me.id))
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(
            edit.contains(r#"data-testid="user-image-preview""#),
            "{edit}"
        );
        assert_eq!(
            edit.matches(r#"data-testid="user-image-preview""#).count(),
            1
        );

        let dashboard = server
            .get("/admin")
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(
            dashboard.contains(r#"data-testid="admin-avatar""#),
            "the admin navbar shows the image"
        );

        let bad = upload(b"not an image".to_vec(), "x.png").await;
        assert!(bad
            .header("location")
            .to_str()
            .unwrap()
            .contains("error=image-type"));

        post(
            &server,
            &format!("/admin/users/{}/image/remove", me.id),
            &cookie,
        )
        .await;
        let after = server
            .get(&format!("/admin/users/{}/edit", me.id))
            .add_header("cookie", cookie)
            .await
            .text();
        assert!(after.contains(r#"data-testid="user-image-none""#));
        assert!(!after.contains(r#"data-testid="admin-avatar""#));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn role_permission_pages_cover_the_standard_screens_and_assign_roles() {
    request::<rust_starter::app::App, _, _>(|server, ctx| async move {
        user(&ctx, "boss@example.com", Some("developer")).await;
        user(&ctx, "staff@example.com", None).await;
        let cookie = login(&server, "boss@example.com", "password123").await;
        let staff = found(&ctx, "staff@example.com").await.unwrap();
        let boss = found(&ctx, "boss@example.com").await.unwrap();
        let base = "/admin/role-permissions";

        for path in [
            "",
            "/roles",
            "/permissions",
            "/permissions-with-role-info",
            "/users",
            "/locations",
            "/users?q=staff",
        ] {
            let page = server
                .get(&format!("{base}{path}"))
                .add_header("cookie", cookie.clone())
                .await;
            assert_eq!(page.status_code(), 200, "{path}");
            let html = page.text();
            assert!(
                html.contains(r#"data-testid="rp-tabs""#)
                    && html.contains(r#"data-testid="breadcrumb""#),
                "{path}"
            );
        }
        let one_role = server
            .get(&format!("{base}/roles/1"))
            .add_header("cookie", cookie.clone())
            .await;
        one_role.assert_status_ok();
        let searched = server
            .get(&format!("{base}/users?q=staff"))
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(searched.contains("staff@example.com"));
        assert_eq!(
            searched.matches(r#"data-testid="id-link""#).count(),
            1,
            "search narrows the table to one row"
        );
        let links = server
            .get(&format!("{base}/locations"))
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(links.contains("/admin/users") && links.contains("roles.view"));

        // Role plus a direct permission, then change it back.
        let assign = |body: &'static str| {
            post(&server, &format!("{base}/users/{}", staff.id), &cookie)
                .content_type("application/x-www-form-urlencoded")
                .bytes(body.into())
        };
        assign("role=marketing&permissions=activity_logs.view&permissions=queue.view").await;
        let access = module_role_permission::access_for(&ctx.db, staff.id)
            .await
            .unwrap();
        assert!(
            access.has_role("marketing")
                && access.can("activity_logs.view")
                && access.can("queue.view")
        );
        assign("role=marketing&permissions=activity_logs.view").await;
        let access = module_role_permission::access_for(&ctx.db, staff.id)
            .await
            .unwrap();
        assert!(access.can("activity_logs.view") && !access.can("queue.view"));

        // Refusals: role-management permissions, unknown role, and your own account.
        for body in [
            "role=&permissions=roles.sync",
            "role=ghost",
            "role=employee&permissions=nope.nope",
        ] {
            let response = assign(body).await;
            assert!(
                response
                    .header("location")
                    .to_str()
                    .unwrap()
                    .contains("error="),
                "{body}"
            );
        }
        let own = post(&server, &format!("{base}/users/{}", boss.id), &cookie)
            .content_type("application/x-www-form-urlencoded")
            .bytes("role=".into())
            .await;
        assert!(own
            .header("location")
            .to_str()
            .unwrap()
            .contains("error=self"));
        assert!(module_role_permission::access_for(&ctx.db, boss.id)
            .await
            .unwrap()
            .has_role("developer"));

        let synced = post(&server, &format!("{base}/sync"), &cookie).await;
        assert!(synced
            .header("location")
            .to_str()
            .unwrap()
            .contains("saved=synced"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn every_admin_list_starts_with_a_clickable_id_column() {
    request::<rust_starter::app::App, _, _>(|server, ctx| async move {
        user(&ctx, "boss@example.com", Some("developer")).await;
        let cookie = login(&server, "boss@example.com", "password123").await;
        let me = found(&ctx, "boss@example.com").await.unwrap();
        {
            use sea_orm::{ActiveModelTrait, Set};
            module_pages::pages::ActiveModel {
                slug: Set("about".into()),
                is_published: Set(true),
                sort_order: Set(0),
                ..Default::default()
            }
            .insert(&ctx.db)
            .await
            .unwrap();
        }

        for (path, target) in [
            ("/admin/users", format!(r#"href="/admin/users/{}""#, me.id)),
            (
                "/admin/role-permissions/roles",
                r#"href="/admin/role-permissions/roles/"#.to_string(),
            ),
            (
                "/admin/role-permissions/users",
                format!(r#"href="/admin/users/{}""#, me.id),
            ),
            ("/admin/pages", r#"href="/admin/pages/"#.to_string()),
        ] {
            let html = server
                .get(path)
                .add_header("cookie", cookie.clone())
                .await
                .text();
            let first_header = html.split("<th ").nth(1).unwrap_or_default();
            assert!(
                first_header.contains("ID</th>"),
                "{path}: the first column is ID"
            );
            let link = html
                .split(r#"data-testid="id-link""#)
                .next()
                .unwrap_or_default();
            assert!(
                html.contains(r#"data-testid="id-link""#),
                "{path}: has a clickable ID"
            );
            assert!(
                html.contains(&target),
                "{path}: the ID links to the show page ({link})"
            );
        }
    })
    .await;
}
