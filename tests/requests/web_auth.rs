use loco_rs::{testing::prelude::*, TestServer};
use rust_starter::{
    app::App,
    seeders::demo_user::{self, DEMO_EMAIL, DEMO_PASSWORD},
};
use serial_test::serial;

const ORIGIN: &str = "http://localhost";

/// Logs in through the real form and returns the `session=...` cookie pair.
async fn login(server: &TestServer, email: &str, password: &str) -> String {
    let response = server
        .post("/login")
        .add_header("host", "localhost")
        .add_header("origin", ORIGIN)
        .form(&[("email", email), ("password", password)])
        .await;
    assert_eq!(
        response.status_code(),
        303,
        "login should redirect, body: {}",
        response.text()
    );
    let set_cookie = response.header("set-cookie").to_str().unwrap().to_string();
    set_cookie.split(';').next().unwrap().to_string()
}

#[tokio::test]
#[serial]
async fn developer_logs_in_and_sees_the_admin_link_and_dashboard() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        let cookie = login(&server, DEMO_EMAIL, DEMO_PASSWORD).await;

        let home = server
            .get("/")
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(
            home.contains(r#"data-testid="user-menu""#),
            "signed-in header shows the user menu"
        );
        assert!(
            home.contains(r#"href="/admin""#),
            "developer sees the admin dashboard link"
        );
        assert!(home.contains("Demo User") && home.contains("Developer"));
        assert!(!home.contains(r#"data-testid="login-link""#));

        let admin = server.get("/admin").add_header("cookie", cookie).await;
        admin.assert_status_ok();
        let html = admin.text();
        assert!(
            html.contains("All navigation links"),
            "the dashboard is the navigation hub"
        );
        assert!(
            !html.contains("Newest users") && !html.contains(r#"data-testid="stats""#),
            "no stat cards or user table"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn session_lookup_finds_a_fresh_session() {
    request::<App, _, _>(|_server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        let user = rust_starter::models::users::Model::find_by_email(&ctx.db, DEMO_EMAIL)
            .await
            .unwrap();
        let token = module_auth::session::create(&ctx.db, user.id, Some("test"))
            .await
            .unwrap();
        let found = module_auth::session::find(&ctx.db, &token).await;
        assert!(
            matches!(found, Ok(Some(_))),
            "lookup result: {:?}",
            found.map(|o| o.map(|(s, _)| s.id))
        );
    })
    .await;
}

async fn user_with_role(ctx: &loco_rs::app::AppContext, email: &str, role: Option<&str>) {
    use rust_starter::models::users::{self, RegisterParams};
    use sea_orm::IntoActiveModel;

    module_role_permission::sync(&ctx.db).await.unwrap();
    let user = users::Model::create_with_password(
        &ctx.db,
        &RegisterParams {
            email: email.to_string(),
            password: "password123".to_string(),
            name: "Test Person".to_string(),
        },
    )
    .await
    .unwrap();
    let user = user.into_active_model().verified(&ctx.db).await.unwrap();
    if let Some(role) = role {
        module_role_permission::assign_role(&ctx.db, user.id, role)
            .await
            .unwrap();
    }
}

#[tokio::test]
#[serial]
async fn guests_see_login_and_register_and_are_sent_to_login_for_admin() {
    request::<App, _, _>(|server, _ctx| async move {
        let home = server.get("/").await.text();
        assert!(
            home.contains(r#"data-testid="login-link""#)
                && home.contains(r#"data-testid="register-link""#)
        );
        assert!(!home.contains(r#"data-testid="user-menu""#));

        let response = server.get("/admin").await;
        assert_eq!(response.status_code(), 303);
        assert_eq!(response.header("location"), "/login?next=/admin");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn wrong_password_and_unknown_email_look_the_same() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        for (email, password) in [
            (DEMO_EMAIL, "wrong-password"),
            ("nobody@example.com", "whatever123"),
        ] {
            let response = server
                .post("/login")
                .add_header("host", "localhost")
                .add_header("origin", ORIGIN)
                .form(&[("email", email), ("password", password)])
                .await;
            assert_eq!(response.status_code(), 401);
            assert!(response.text().contains("Incorrect email or password."));
            assert!(response.maybe_header("set-cookie").is_none());
        }
    })
    .await;
}

#[tokio::test]
#[serial]
async fn form_posts_from_other_sites_are_rejected() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        let form = [("email", DEMO_EMAIL), ("password", DEMO_PASSWORD)];

        let no_origin = server
            .post("/login")
            .add_header("host", "localhost")
            .form(&form)
            .await;
        assert_eq!(no_origin.status_code(), 403);

        let foreign = server
            .post("/login")
            .add_header("host", "localhost")
            .add_header("origin", "https://evil.example")
            .form(&form)
            .await;
        assert_eq!(foreign.status_code(), 403);
        assert!(foreign.maybe_header("set-cookie").is_none());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn five_failed_logins_lock_the_account_for_a_while() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        for _ in 0..5 {
            let response = server
                .post("/login")
                .add_header("host", "localhost")
                .add_header("origin", ORIGIN)
                .form(&[("email", DEMO_EMAIL), ("password", "bad-password")])
                .await;
            assert_eq!(response.status_code(), 401);
        }
        let locked = server
            .post("/login")
            .add_header("host", "localhost")
            .add_header("origin", ORIGIN)
            .form(&[("email", DEMO_EMAIL), ("password", DEMO_PASSWORD)])
            .await;
        assert_eq!(
            locked.status_code(),
            429,
            "even the right password waits out the lockout"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn logout_ends_the_session() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        let cookie = login(&server, DEMO_EMAIL, DEMO_PASSWORD).await;

        let response = server
            .post("/logout")
            .add_header("host", "localhost")
            .add_header("origin", ORIGIN)
            .add_header("cookie", cookie.clone())
            .await;
        assert_eq!(response.status_code(), 303);
        assert!(response
            .header("set-cookie")
            .to_str()
            .unwrap()
            .contains("Max-Age=0"));

        // The old cookie no longer works.
        let after = server.get("/admin").add_header("cookie", cookie).await;
        assert_eq!(after.status_code(), 303);
    })
    .await;
}

#[rstest::rstest]
#[case(
    "A",
    "a@b.co",
    "password123",
    "password123",
    "Enter a name of 2 to 80 characters."
)]
#[case(
    "Ada Lovelace",
    "not-an-email",
    "password123",
    "password123",
    "Enter a valid email address."
)]
#[case(
    "Ada Lovelace",
    "ada@example.com",
    "short",
    "short",
    "The password must be at least 8 characters."
)]
#[case(
    "Ada Lovelace",
    "ada@example.com",
    "password123",
    "different123",
    "The two passwords do not match."
)]
#[tokio::test]
#[serial]
async fn register_validates_input(
    #[case] name: &str,
    #[case] email: &str,
    #[case] password: &str,
    #[case] confirm: &str,
    #[case] message: &str,
) {
    request::<App, _, _>(|server, _ctx| async move {
        let response = server
            .post("/register")
            .add_header("host", "localhost")
            .add_header("origin", ORIGIN)
            .form(&[
                ("name", name),
                ("email", email),
                ("password", password),
                ("password_confirmation", confirm),
            ])
            .await;
        assert_eq!(response.status_code(), 422);
        assert!(response.text().contains(message), "expected `{message}`");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn register_creates_a_signed_in_account_without_any_role() {
    request::<App, _, _>(|server, _ctx| async move {
        let form = [
            ("name", "Ada Lovelace"),
            ("email", "Ada@Example.com"),
            ("password", "password123"),
            ("password_confirmation", "password123"),
        ];
        let response = server
            .post("/register")
            .add_header("host", "localhost")
            .add_header("origin", ORIGIN)
            .form(&form)
            .await;
        assert_eq!(response.status_code(), 303);
        let cookie = response
            .header("set-cookie")
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();

        let home = server
            .get("/")
            .add_header("cookie", cookie.clone())
            .await
            .text();
        assert!(
            home.contains("Ada Lovelace") && home.contains("ada@example.com"),
            "email is stored lowercase"
        );
        assert!(
            !home.contains(r#"href="/admin""#),
            "a new account has no role, so no admin link"
        );
        assert_eq!(
            server
                .get("/admin")
                .add_header("cookie", cookie)
                .await
                .status_code(),
            403,
            "no role means no dashboard"
        );

        let again = server
            .post("/register")
            .add_header("host", "localhost")
            .add_header("origin", ORIGIN)
            .form(&form)
            .await;
        assert_eq!(again.status_code(), 409);
        assert!(again
            .text()
            .contains("An account with this email already exists."));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn sidebar_and_pages_follow_the_role_hierarchy() {
    request::<App, _, _>(|server, ctx| async move {
        user_with_role(&ctx, "new@example.com", Some("new_employee")).await;
        user_with_role(&ctx, "admin@example.com", Some("admin")).await;

        let cookie = login(&server, "new@example.com", "password123").await;
        let dashboard = server
            .get("/admin")
            .add_header("cookie", cookie.clone())
            .await;
        dashboard.assert_status_ok();
        let html = dashboard.text();
        assert!(html.contains(r#"data-navigation-label="Dashboard""#));
        assert!(
            !html.contains(r#"data-navigation-label="Users""#),
            "new employee has no users.view"
        );
        assert_eq!(
            server
                .get("/admin/users")
                .add_header("cookie", cookie)
                .await
                .status_code(),
            403
        );

        let cookie = login(&server, "admin@example.com", "password123").await;
        let users = server
            .get("/admin/users")
            .add_header("cookie", cookie.clone())
            .await;
        users.assert_status_ok();
        assert!(
            !users.text().contains(r#"action="/admin/users/"#),
            "admin cannot assign roles"
        );
        assert_eq!(
            server
                .get("/admin/role-permissions")
                .add_header("cookie", cookie)
                .await
                .status_code(),
            403,
            "roles manager is developer only"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn developer_can_manage_roles_but_not_their_own() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        user_with_role(&ctx, "staff@example.com", None).await;
        let cookie = login(&server, DEMO_EMAIL, DEMO_PASSWORD).await;

        let roles = server
            .get("/admin/role-permissions/roles")
            .add_header("cookie", cookie.clone())
            .await;
        roles.assert_status_ok();
        let matrix = server
            .get("/admin/role-permissions/permissions-with-role-info")
            .add_header("cookie", cookie.clone())
            .await;
        matrix.assert_status_ok();
        let html = roles.text() + &matrix.text();
        assert!(
            html.contains("Business Development")
                && html.contains("Marketing")
                && html.contains("dashboard.view")
        );

        let staff = rust_starter::models::users::Model::find_by_email(&ctx.db, "staff@example.com")
            .await
            .unwrap();
        let me = rust_starter::models::users::Model::find_by_email(&ctx.db, DEMO_EMAIL)
            .await
            .unwrap();

        let assign = |id: i64, role: &'static str| {
            let server = &server;
            let cookie = cookie.clone();
            async move {
                server
                    .post(&format!("/admin/users/{id}/role"))
                    .add_header("host", "localhost")
                    .add_header("origin", ORIGIN)
                    .add_header("cookie", cookie)
                    .form(&[("role", role)])
                    .await
            }
        };

        assert_eq!(assign(staff.id, "marketing").await.status_code(), 303);
        let access = module_role_permission::access_for(&ctx.db, staff.id)
            .await
            .unwrap();
        assert!(
            access.has_role("marketing")
                && access.can("pages.manage")
                && access.can("users.view")
                && !access.can("users.manage")
        );

        assert_eq!(assign(staff.id, "").await.status_code(), 303);
        assert!(module_role_permission::access_for(&ctx.db, staff.id)
            .await
            .unwrap()
            .role
            .is_none());

        let own = assign(me.id, "employee").await;
        assert_eq!(own.header("location"), "/admin/users?error=self");
        assert!(module_role_permission::access_for(&ctx.db, me.id)
            .await
            .unwrap()
            .has_role("developer"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn quick_search_data_lists_only_permitted_sidebar_items() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        let cookie = login(&server, DEMO_EMAIL, DEMO_PASSWORD).await;
        let html = server
            .get("/admin")
            .add_header("cookie", cookie)
            .await
            .text();
        assert!(html.contains(r#"data-testid="quick-nav""#));
        for label in [
            "Dashboard",
            "Users",
            "Roles and permissions",
            "Scheduled jobs",
        ] {
            assert!(
                html.contains(&format!(r#"data-navigation-label="{label}""#)),
                "missing sidebar item {label}"
            );
        }
        assert!(
            html.contains("People and access")
                && html.contains("System")
                && html.contains("Overview")
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn dashboard_hub_lists_every_permitted_sidebar_link_with_counts() {
    request::<App, _, _>(|server, ctx| async move {
        demo_user::run(&ctx.db).await.unwrap();
        user_with_role(&ctx, "new@example.com", Some("new_employee")).await;
        user_with_role(&ctx, "market@example.com", Some("marketing")).await;

        // Developer: every area.
        let cookie = login(&server, DEMO_EMAIL, DEMO_PASSWORD).await;
        let html = server
            .get("/admin")
            .add_header("cookie", cookie)
            .await
            .text();
        assert!(html.contains(r#"data-testid="nav-hub""#) && html.contains("All navigation links"));
        assert!(
            html.contains("4 sections") && html.contains("10 links"),
            "overview, content, people and access, system"
        );
        for label in [
            "Dashboard",
            "Pages",
            "Media library",
            "Contacts",
            "Users",
            "Roles and permissions",
            "Scheduled jobs",
            "Activity log",
            "Queue monitor",
            "AI API logs",
        ] {
            assert!(
                html.contains(&format!(r#"data-nav-link data-label="{label}""#)),
                "hub is missing {label}"
            );
        }
        assert!(
            html.contains(r#"data-testid="nav-search""#),
            "the hub has its search box"
        );
        assert!(
            html.contains(r#"href="/admin/media""#) && html.contains("fa-regular fa-images"),
            "links carry their icon"
        );

        // New employee: only the dashboard.
        let cookie = login(&server, "new@example.com", "password123").await;
        let html = server
            .get("/admin")
            .add_header("cookie", cookie)
            .await
            .text();
        assert!(html.contains("1 sections") && html.contains("1 links"));
        assert!(html.contains(r#"data-nav-link data-label="Dashboard""#));
        assert!(
            !html.contains(r#"data-nav-link data-label="Users""#)
                && !html.contains(r#"data-nav-link data-label="Media library""#)
        );

        // Marketing: dashboard, pages, users, media library (media.view) but not roles or the schedule.
        let cookie = login(&server, "market@example.com", "password123").await;
        let html = server
            .get("/admin")
            .add_header("cookie", cookie)
            .await
            .text();
        for label in ["Dashboard", "Pages", "Users", "Media library", "Contacts"] {
            assert!(
                html.contains(&format!(r#"data-nav-link data-label="{label}""#)),
                "marketing should see {label}"
            );
        }
        for label in ["Roles and permissions", "Scheduled jobs"] {
            assert!(
                !html.contains(&format!(r#"data-nav-link data-label="{label}""#)),
                "marketing must not see {label}"
            );
        }
        assert!(html.contains("5 links"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn staff_get_an_open_in_admin_button_on_the_public_page() {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    request::<App, _, _>(|server, ctx| async move {
        loco_modular::seed_all(&modules_registry::all(), &ctx)
            .await
            .unwrap();
        user_with_role(&ctx, "market@example.com", Some("marketing")).await; // pages.manage
        user_with_role(&ctx, "biz@example.com", Some("business_development")).await; // pages.view only
        user_with_role(&ctx, "new@example.com", Some("new_employee")).await; // no pages permission
        user_with_role(&ctx, "plain@example.com", None).await;
        let id = module_pages::pages::Entity::find()
            .filter(module_pages::pages::Column::Slug.eq("about"))
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap()
            .id;
        let show_url = format!(r#"href="/admin/pages/{id}""#);

        // A guest first, so the page is cached before any staff member looks at it.
        let guest = server.get("/pages/about").await.text();
        assert!(!guest.contains(r#"data-testid="admin-page-link""#));

        for (email, who) in [
            ("market@example.com", "marketing"),
            ("biz@example.com", "business development"),
        ] {
            let cookie = login(&server, email, "password123").await;
            let html = server
                .get("/pages/about")
                .add_header("cookie", cookie.clone())
                .await
                .text();
            assert!(
                html.contains(r#"data-testid="admin-page-link""#) && html.contains(&show_url),
                "{who} gets the button"
            );
            assert!(html.contains("Open in admin"));
            assert!(
                !html.contains(&format!(r#"href="/admin/pages/{id}/edit""#)),
                "the button opens the show page, not the editor"
            );
            assert!(
                html.contains("fixed bottom-5 right-5"),
                "it floats instead of taking a row of the page"
            );
            let bangla = server
                .get("/pages/about?lang=bn")
                .add_header("cookie", cookie)
                .await
                .text();
            assert!(bangla.contains("অ্যাডমিনে খুলুন") && bangla.contains(&show_url));
        }

        for email in ["new@example.com", "plain@example.com"] {
            let cookie = login(&server, email, "password123").await;
            let html = server
                .get("/pages/about")
                .add_header("cookie", cookie)
                .await
                .text();
            assert!(
                !html.contains(r#"data-testid="admin-page-link""#) && !html.contains(&show_url),
                "{email} must not see it"
            );
        }

        // The button is not baked into the cached page: a guest still sees none afterwards.
        let guest_again = server.get("/pages/about").await.text();
        assert!(
            !guest_again.contains(r#"data-testid="admin-page-link""#)
                && !guest_again.contains(&show_url)
        );
    })
    .await;
}
