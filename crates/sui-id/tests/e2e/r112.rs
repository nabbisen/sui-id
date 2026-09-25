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

fn schema_item(body: &str) -> String {
    let at = body.find("スキーマバージョン").expect("the schema label");
    body[at..].chars().take(160).collect()
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
