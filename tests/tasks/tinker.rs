use loco_rs::testing::prelude::*;
use rust_starter::{
    app::App,
    tasks::tinker::{Reply, Session},
};
use serial_test::serial;

async fn rows(
    session: &mut Session,
    ctx: &loco_rs::app::AppContext,
    sql: &str,
) -> Vec<Vec<String>> {
    match session.handle(ctx, sql, false).await.unwrap() {
        Reply::Table { rows, .. } => rows,
        other => panic!("expected a table, got {other:?}"),
    }
}

#[tokio::test]
#[serial]
async fn the_sql_console_reads_writes_and_can_roll_back() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    let mut session = Session::new(false);

    session
        .handle(&ctx, "insert into pages (slug, is_published, sort_order, created_at, updated_at) values ('tinker-demo', true, 0, now(), now());", false)
        .await
        .unwrap();
    assert_eq!(
        rows(
            &mut session,
            &ctx,
            "select slug from pages where slug = 'tinker-demo';"
        )
        .await,
        vec![vec!["tinker-demo".to_string()]]
    );

    // Inside a transaction a change is visible to the console, then thrown away.
    session.handle(&ctx, ".begin", false).await.unwrap();
    assert!(session.in_transaction());
    let reply = session
        .handle(
            &ctx,
            "update pages set sort_order = 9 where slug = 'tinker-demo'",
            false,
        )
        .await
        .unwrap();
    assert!(
        matches!(&reply, Reply::Message(text) if text.contains("1 row(s) affected") && text.contains("not saved yet"))
    );
    assert_eq!(
        rows(
            &mut session,
            &ctx,
            "select sort_order from pages where slug = 'tinker-demo'"
        )
        .await[0][0],
        "9"
    );
    session.handle(&ctx, ".rollback", false).await.unwrap();
    assert!(!session.in_transaction());
    assert_eq!(
        rows(
            &mut session,
            &ctx,
            "select sort_order from pages where slug = 'tinker-demo'"
        )
        .await[0][0],
        "0"
    );

    // A commit keeps it.
    session.handle(&ctx, ".begin", false).await.unwrap();
    session
        .handle(
            &ctx,
            "update pages set sort_order = 5 where slug = 'tinker-demo'",
            false,
        )
        .await
        .unwrap();
    session.handle(&ctx, ".commit", false).await.unwrap();
    assert_eq!(
        rows(
            &mut session,
            &ctx,
            "select sort_order from pages where slug = 'tinker-demo'"
        )
        .await[0][0],
        "5"
    );

    // Touching every row needs a yes; the answer is passed back in.
    let ask = session
        .handle(&ctx, "delete from pages", false)
        .await
        .unwrap();
    assert!(matches!(ask, Reply::Confirm(_)));
    assert_eq!(rows(&mut session, &ctx, ".count pages").await[0][0], "1");
    session
        .handle(&ctx, "delete from pages", true)
        .await
        .unwrap();
    assert_eq!(rows(&mut session, &ctx, ".count pages").await[0][0], "0");

    // Helpers and mistakes.
    assert!(
        matches!(session.handle(&ctx, ".hash secret", false).await.unwrap(), Reply::Message(hash) if hash.starts_with("$argon2"))
    );
    assert!(session
        .handle(&ctx, ".desc users; drop table users", false)
        .await
        .is_err());
    assert!(session.handle(&ctx, ".commit", false).await.is_err());
    assert!(session
        .handle(&ctx, "select * from no_such_table", false)
        .await
        .is_err());
    assert!(matches!(
        session.handle(&ctx, ".exit", false).await.unwrap(),
        Reply::Exit
    ));
}

#[tokio::test]
#[serial]
async fn read_only_mode_refuses_changes_but_allows_reading() {
    let ctx = boot_test::<App>().await.unwrap().app_context;
    let mut session = Session::new(true);
    assert!(session
        .handle(&ctx, "update users set name = 'x' where id = 1", false)
        .await
        .is_err());
    assert!(session.handle(&ctx, "select 1", false).await.is_ok());
    assert!(session.handle(&ctx, ".tables", false).await.is_ok());
}
