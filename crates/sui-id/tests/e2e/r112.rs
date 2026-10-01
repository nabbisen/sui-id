//! RFC 112 stage 1, the settings page: Settings → Advanced shows the schema
//! version the database **records**, not the binary's ceiling. The two are equal
//! on a current database, so the test moves the stored one.

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;
use tower::ServiceExt;

async fn other_page(state: &sui_id::AppState, session: &str) -> String {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/settings/other")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("other");
    assert_eq!(resp.status(), StatusCode::OK);
    String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned()
}

/// The schema-version table cell's own content — not a fixed-width window
/// after the label, which used to sweep past the cell and into whatever
/// rendered next (the server clock, in production), making the assertion a
/// bare substring search over text it was never meant to cover. See the
/// unit test below for the exact case that broke: a clock reading that
/// happens to contain the schema ceiling's digits.
fn schema_item(body: &str) -> String {
    let at = body.find("スキーマバージョン").expect("the schema label");
    let rest = &body[at..];
    let td_open = rest.find("<td>").expect("the schema value cell") + "<td>".len();
    let td_close = rest[td_open..]
        .find("</td>")
        .expect("the schema value cell's close tag");
    rest[td_open..td_open + td_close].to_owned()
}

#[test]
fn schema_item_reads_only_the_label_cell_not_a_trailing_clock() {
    // The exact shape that made the old 160-character-window assertion
    // fail roughly one run in thirty (CI run 36711349086, clock reading
    // 12:07:43 UTC): the schema cell is correct, but a fixed window swept
    // past it into the next row, whose rendered clock happened to contain
    // "43" — MAX_SCHEMA_VERSION — making `contains(&ceiling.to_string())`
    // true for a page that did not show the ceiling at all.
    let page = concat!(
        r#"<tr><th scope="row" class="kv-label-cell">スキーマバージョン</th>"#,
        r#"<td><span>7</span></td></tr>"#,
        r#"<tr><th scope="row" class="kv-label-cell">サーバ時刻</th>"#,
        r#"<td><span class="code">2026-09-30 15:41:43 UTC</span></td></tr>"#,
    );
    let cell = schema_item(page);
    assert!(cell.contains('7'), "the schema cell's own value: {cell}");
    assert!(
        !cell.contains("43"),
        "the extracted cell must not reach the clock row at all: {cell}"
    );
}

#[tokio::test]
async fn r112_settings_shows_the_stored_schema_version_not_the_binarys_ceiling() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let ceiling = sui_id_store::migrations::MAX_SCHEMA_VERSION;

    let before = schema_item(&other_page(&state, &session).await);
    assert!(
        before.contains(&ceiling.to_string()),
        "a current database shows {ceiling}: {before}"
    );

    state
        .db
        .with_conn(|c| {
            c.execute_batch("UPDATE sui_meta SET value='7' WHERE key='schema_version'")?;
            Ok(())
        })
        .await
        .expect("stamp");
    let after = schema_item(&other_page(&state, &session).await);
    assert!(after.contains('7'), "the stored value is shown: {after}");
    assert!(
        !after.contains(&ceiling.to_string()),
        "the ceiling is not what is shown: {after}"
    );
}
