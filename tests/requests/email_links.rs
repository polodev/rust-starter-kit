use loco_rs::{app::AppContext, testing::prelude::*, TestServer};
use module_mail::{captured, clear_captured};
use rust_starter::app::App;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serial_test::serial;

const ORIGIN: &str = "http://localhost";

async fn post_form(
    server: &TestServer,
    path: &str,
    form: &[(&str, &str)],
) -> axum_test::TestResponse {
    server
        .post(path)
        .add_header("host", "localhost")
        .add_header("origin", ORIGIN)
        .form(&form)
        .await
}

async fn register(server: &TestServer, email: &str) -> String {
    let response = post_form(
        server,
        "/register",
        &[
            ("name", "Ada Lovelace"),
            ("email", email),
            ("password", "password123"),
            ("password_confirmation", "password123"),
        ],
    )
    .await;
    assert_eq!(response.status_code(), 303, "{}", response.text());
    assert_eq!(response.header("location"), "/email/verify");
    response
        .header("set-cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

/// The first `{prefix}...` link in the newest captured mail, as a path.
fn link_in_last_mail(prefix: &str) -> String {
    let raw = String::from_utf8_lossy(&captured().last().expect("a mail was sent").raw)
        .replace("=\r\n", "")
        .replace("=\n", "");
    let start = raw
        .find(prefix)
        .unwrap_or_else(|| panic!("no `{prefix}` link in the mail:\n{raw}"));
    let rest = &raw[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || "/-_".contains(c)))
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

async fn login_status(server: &TestServer, email: &str, password: &str) -> (u16, String) {
    let response = post_form(
        server,
        "/login",
        &[("email", email), ("password", password)],
    )
    .await;
    (response.status_code().as_u16(), response.text())
}

async fn verified(ctx: &AppContext, email: &str) -> bool {
    module_auth::users::Entity::find()
        .filter(module_auth::users::Column::Email.eq(email))
        .one(&ctx.db)
        .await
        .unwrap()
        .unwrap()
        .email_verified_at
        .is_some()
}

#[tokio::test]
#[serial]
async fn sign_up_mails_a_link_and_pages_that_need_a_verified_address_wait_for_it() {
    request::<App, _, _>(|server, ctx| async move {
        clear_captured();
        let cookie = register(&server, "Ada@Example.com").await;
        assert!(!verified(&ctx, "ada@example.com").await);
        assert_eq!(captured().len(), 1);
        assert_eq!(captured()[0].recipients, vec!["ada@example.com"]);

        // Signed in, but not verified: like Laravel's `verified` middleware.
        let notice = server
            .get("/email/verify")
            .add_header("cookie", cookie.clone())
            .await;
        notice.assert_status_ok();
        assert!(notice.text().contains(r#"data-testid="verify-notice""#));
        for gated in ["/admin", "/account/password", "/account/logo"] {
            let response = server.get(gated).add_header("cookie", cookie.clone()).await;
            assert_eq!(response.status_code(), 303, "{gated}");
            assert_eq!(response.header("location"), "/email/verify", "{gated}");
        }
        let profile = server
            .get("/account/profile")
            .add_header("cookie", cookie.clone())
            .await;
        profile.assert_status_ok();
        assert!(profile
            .text()
            .contains(r#"data-testid="email-not-verified""#));
        assert_eq!(
            login_status(&server, "ada@example.com", "password123")
                .await
                .0,
            303,
            "an unverified person may sign in"
        );
        assert_eq!(
            login_status(&server, "ada@example.com", "wrong-password")
                .await
                .0,
            401
        );

        let link = link_in_last_mail("/email/verify/");
        let response = server.get(&link).await;
        assert_eq!(response.status_code(), 303);
        assert!(response
            .header("location")
            .to_str()
            .unwrap()
            .contains("login-email-verified"));
        assert!(verified(&ctx, "ada@example.com").await);
        assert_eq!(
            server
                .get("/email/verify")
                .add_header("cookie", cookie.clone())
                .await
                .header("location"),
            "/"
        );
        assert_ne!(
            server
                .get("/account/password")
                .add_header("cookie", cookie.clone())
                .await
                .status_code(),
            303
        );
        assert!(server
            .get("/account/profile")
            .add_header("cookie", cookie)
            .await
            .text()
            .contains(r#"data-testid="email-verified""#));

        // The link works once.
        let again = server.get(&link).await;
        assert!(again
            .header("location")
            .to_str()
            .unwrap()
            .contains("login-link-invalid"));
        assert!(server
            .get("/login?notice=login-email-verified")
            .await
            .text()
            .contains("Your email address is verified."));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn the_verification_mail_can_be_asked_for_again_but_never_leaks_who_has_an_account() {
    request::<App, _, _>(|server, ctx| async move {
        clear_captured();
        register(&server, "ada@example.com").await;
        clear_captured();

        let page = server.get("/email/verification-notification").await;
        page.assert_status_ok();
        let sent = post_form(
            &server,
            "/email/verification-notification",
            &[("email", "ada@example.com")],
        )
        .await;
        assert!(sent.text().contains(r#"data-testid="link-sent""#));
        assert_eq!(captured().len(), 1);

        let unknown = post_form(
            &server,
            "/email/verification-notification",
            &[("email", "nobody@example.com")],
        )
        .await;
        assert!(
            unknown.text().contains(r#"data-testid="link-sent""#),
            "same answer for an unknown address"
        );
        assert_eq!(captured().len(), 1, "nothing is sent to an unknown address");

        let forged = server
            .post("/email/verification-notification")
            .add_header("host", "localhost")
            .add_header("origin", "http://evil.example")
            .form(&[("email", "ada@example.com")])
            .await;
        assert_eq!(forged.status_code(), 403);

        let link = link_in_last_mail("/email/verify/");
        server.get(&link).await;
        assert!(verified(&ctx, "ada@example.com").await);
        // Already verified: no second mail.
        post_form(
            &server,
            "/email/verification-notification",
            &[("email", "ada@example.com")],
        )
        .await;
        assert_eq!(captured().len(), 1);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_forgotten_password_is_reset_with_a_one_time_link_and_old_sessions_end() {
    request::<App, _, _>(|server, ctx| async move {
        clear_captured();
        register(&server, "ada@example.com").await;
        server.get(&link_in_last_mail("/email/verify/")).await;
        let signed_in = post_form(
            &server,
            "/login",
            &[("email", "ada@example.com"), ("password", "password123")],
        )
        .await;
        let old_cookie = signed_in
            .header("set-cookie")
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        assert!(server
            .get("/")
            .add_header("cookie", old_cookie.clone())
            .await
            .text()
            .contains(r#"data-testid="user-menu""#));

        clear_captured();
        server.get("/forgot-password").await.assert_status_ok();
        let unknown = post_form(
            &server,
            "/forgot-password",
            &[("email", "nobody@example.com")],
        )
        .await;
        assert!(unknown.text().contains(r#"data-testid="link-sent""#));
        assert!(captured().is_empty());
        post_form(&server, "/forgot-password", &[("email", "ada@example.com")]).await;
        let link = link_in_last_mail("/reset-password/");

        server.get(&link).await.assert_status_ok();
        let short = post_form(
            &server,
            &link,
            &[("password", "short"), ("password_confirmation", "short")],
        )
        .await;
        assert_eq!(short.status_code(), 422);
        let mismatch = post_form(
            &server,
            &link,
            &[
                ("password", "newpassword1"),
                ("password_confirmation", "newpassword2"),
            ],
        )
        .await;
        assert_eq!(mismatch.status_code(), 422);
        let forged = server
            .post(&link)
            .add_header("host", "localhost")
            .add_header("origin", "http://evil.example")
            .form(&[
                ("password", "newpassword1"),
                ("password_confirmation", "newpassword1"),
            ])
            .await;
        assert_eq!(forged.status_code(), 403);
        assert_eq!(
            login_status(&server, "ada@example.com", "password123")
                .await
                .0,
            303,
            "nothing changed after refusals"
        );

        let done = post_form(
            &server,
            &link,
            &[
                ("password", "newpassword1"),
                ("password_confirmation", "newpassword1"),
            ],
        )
        .await;
        assert_eq!(done.status_code(), 303);
        assert!(done
            .header("location")
            .to_str()
            .unwrap()
            .contains("login-password-reset-done"));
        assert_eq!(
            login_status(&server, "ada@example.com", "password123")
                .await
                .0,
            401,
            "old password is dead"
        );
        assert_eq!(
            login_status(&server, "ada@example.com", "newpassword1")
                .await
                .0,
            303
        );
        assert!(
            !server
                .get("/")
                .add_header("cookie", old_cookie)
                .await
                .text()
                .contains(r#"data-testid="user-menu""#),
            "an old session was ended"
        );

        // The link is used up.
        let reused = post_form(
            &server,
            &link,
            &[
                ("password", "another-pass1"),
                ("password_confirmation", "another-pass1"),
            ],
        )
        .await;
        assert!(reused
            .header("location")
            .to_str()
            .unwrap()
            .contains("login-link-invalid"));
        assert!(server
            .get(&link)
            .await
            .header("location")
            .to_str()
            .unwrap()
            .contains("login-link-invalid"));
        let _ = ctx;
    })
    .await;
}

#[tokio::test]
#[serial]
async fn an_expired_link_is_refused() {
    request::<App, _, _>(|server, ctx| async move {
        clear_captured();
        register(&server, "ada@example.com").await;
        let link = link_in_last_mail("/email/verify/");
        let token = module_auth::user_security_tokens::Entity::find()
            .one(&ctx.db)
            .await
            .unwrap()
            .unwrap();
        let mut expired = token.into_active_model();
        expired.expires_at =
            Set((chrono::Utc::now() - chrono::Duration::minutes(1)).fixed_offset());
        sea_orm::ActiveModelTrait::update(expired, &ctx.db)
            .await
            .unwrap();

        let response = server.get(&link).await;
        assert!(response
            .header("location")
            .to_str()
            .unwrap()
            .contains("login-link-invalid"));
        assert!(!verified(&ctx, "ada@example.com").await);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn a_sign_in_link_logs_in_without_a_password_and_works_once() {
    request::<App, _, _>(|server, ctx| async move {
        clear_captured();
        register(&server, "ada@example.com").await;
        clear_captured();

        server.get("/login/email-link").await.assert_status_ok();
        let unknown = post_form(
            &server,
            "/login/email-link",
            &[("email", "nobody@example.com")],
        )
        .await;
        assert!(unknown.text().contains(r#"data-testid="link-sent""#));
        assert!(captured().is_empty());
        post_form(
            &server,
            "/login/email-link",
            &[("email", "ada@example.com")],
        )
        .await;
        let link = link_in_last_mail("/login/email-link/");

        // Opening the link only shows a button (mail scanners open links); it does not sign anyone in.
        let page = server.get(&link).await;
        page.assert_status_ok();
        assert!(page
            .text()
            .contains(r#"data-testid="sign-in-link-confirm""#));
        assert!(page.maybe_header("set-cookie").is_none());

        let forged = server
            .post(&link)
            .add_header("host", "localhost")
            .add_header("origin", "http://evil.example")
            .await;
        assert_eq!(forged.status_code(), 403);

        let signed_in = post_form(&server, &link, &[]).await;
        assert_eq!(signed_in.status_code(), 303);
        let cookie = signed_in
            .header("set-cookie")
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        assert!(server
            .get("/")
            .add_header("cookie", cookie)
            .await
            .text()
            .contains("Ada Lovelace"));
        assert!(
            verified(&ctx, "ada@example.com").await,
            "a mailbox-proven sign-in verifies the address"
        );

        let again = post_form(&server, &link, &[]).await;
        assert!(again
            .header("location")
            .to_str()
            .unwrap()
            .contains("login-link-invalid"));
        assert!(again.maybe_header("set-cookie").is_none());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn link_requests_are_limited_per_address() {
    request::<App, _, _>(|server, _ctx| async move {
        clear_captured();
        register(&server, "ada@example.com").await;
        clear_captured();
        for _ in 0..5 {
            post_form(&server, "/forgot-password", &[("email", "ada@example.com")]).await;
        }
        assert_eq!(
            captured().len(),
            3,
            "only three mails an hour for one address"
        );
    })
    .await;
}

#[tokio::test]
#[serial]
async fn the_login_page_offers_both_links_in_both_languages() {
    request::<App, _, _>(|server, _ctx| async move {
        let page = server.get("/login").await.text();
        assert!(
            page.contains(r#"data-testid="forgot-password-link""#)
                && page.contains(r#"data-testid="sign-in-link-link""#)
        );
        assert!(page.contains("Forgot your password?"));
    })
    .await;
}
