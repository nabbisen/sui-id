//! RFC 121 — a verification failure is not a pass.
//!
//! Every test drives the real router: `/admin/audit` and
//! `/admin/settings/logs` both call `sui_id_core::audit_chain::check`
//! (through `crate::handlers::chain_status`), so what these assert is what
//! an operator actually sees, not the shared function in isolation
//! (`sui-id-core`'s own unit tests already cover that directly).

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;
use tower::ServiceExt;

use super::common::*;

async fn get(state: &sui_id::AppState, session: &str, path: &str) -> String {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(path)
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("request");
    assert_eq!(resp.status(), StatusCode::OK, "{path}");
    String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned()
}

/// `/admin/audit`'s own entry list (`recent_filtered`, `limit=200`) parses
/// the same `actor` column `verify_chain_tail` does, so corrupting a row
/// inside *both* windows fails the page for a reason that has nothing to do
/// with RFC 121 — the entries fetch, not the chain check. To exercise the
/// chain check in isolation on that page, the corrupted row has to sit
/// outside `recent_filtered`'s window but inside `verify_chain_tail`'s wider
/// one: append enough rows that the oldest is excluded from the first but
/// not the second, and corrupt that one.
async fn corrupt_a_row_outside_the_entry_list_but_inside_the_chain_window(
    state: &sui_id::AppState,
) {
    // recent_filtered's limit is 200; verify_chain_tail's is 500 (audit page)
    // and 100 (settings page — smaller, but this fn is only used where the
    // audit page's own entry list also has to keep working). 205 rows: seq 1
    // is well outside the newest-200 window recent_filtered reads, and well
    // inside any verify_chain_tail window used in this file.
    for i in 0..205 {
        sui_id_store::repos::audit::append(
            &state.db,
            &sui_id_store::models::AuditLogRow {
                at: chrono::Utc::now(),
                actor: None,
                action: format!("test.filler.{i}"),
                target: None,
                result: "ok".into(),
                note: None,
            },
        )
        .await
        .expect("append filler row");
    }
    state
        .db
        .with_conn(|c| {
            c.execute(
                "UPDATE audit_log SET actor = 'not-a-uuid' WHERE seq = 1",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("corrupt actor");
}

/// The simpler injection, for a test that does not also read the entry list
/// (the settings page shows counts only; `check` on its own reads nothing
/// else). Same method RFC 121's own design review used: ordinary SQL, no
/// test-only branch in production code.
async fn corrupt_the_one_audit_row(state: &sui_id::AppState) {
    state
        .db
        .with_conn(|c| {
            c.execute(
                "UPDATE audit_log SET actor = 'not-a-uuid' WHERE seq = 1",
                [],
            )?;
            Ok(())
        })
        .await
        .expect("corrupt actor");
}

#[tokio::test]
async fn both_surfaces_show_intact_when_the_chain_is_intact() {
    // Pages render in the server's default locale (ja) unless a session or
    // server setting picks another; the words checked here are exactly what
    // `chain_status_label`/`chain_status_words` produce for `Locale::Ja`.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let audit = get(&state, &session, "/admin/audit").await;
    let logs = get(&state, &session, "/admin/settings/logs").await;
    assert!(audit.contains("監査チェーンは正常です"), "{audit}");
    assert!(logs.contains(">正常<"), "{logs}");
    for body in [&audit, &logs] {
        assert!(!body.contains("検証できませんでした"), "{body}");
        assert!(!body.contains("失敗しました"), "{body}");
    }
}

#[tokio::test]
async fn the_audit_page_shows_could_not_verify_not_intact_and_not_broken() {
    // fails before RFC 121: the audit page substituted a report meaning
    // "nothing wrong" and showed the same green banner as an intact chain.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    corrupt_a_row_outside_the_entry_list_but_inside_the_chain_window(&state).await;

    let audit = get(&state, &session, "/admin/audit").await;
    assert!(!audit.contains("監査チェーンは正常です"), "{audit}");
    assert!(!audit.contains("失敗しました"), "{audit}");
    assert!(
        audit.contains("監査チェーンを検証できませんでした"),
        "{audit}"
    );
}

#[tokio::test]
async fn settings_logs_shows_could_not_verify_and_still_shows_its_other_counters() {
    // fails before RFC 121: this page failed its entire tab with a 500 on
    // exactly this input (RFC 121 D6). It reads no entry list, so the
    // simplest one-row corruption isolates the chain check on its own —
    // unlike the audit page above, which needed enough rows that the
    // corrupted one sits outside its entry list's window but inside its
    // (wider) chain-check window; there is no row position that does that
    // for *this* page's narrower 100-row window too, which is why the two
    // pages are exercised with different fixtures rather than one shared
    // scenario. Both still assert the identical words (RFC 121 D2).
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    corrupt_the_one_audit_row(&state).await;
    let logs = get(&state, &session, "/admin/settings/logs").await;
    assert!(!logs.contains(">正常<"), "{logs}");
    assert!(!logs.contains(">異常<"), "{logs}");
    assert!(logs.contains("auth.login.success"), "{logs}");
    assert!(logs.contains("検査そのものが完了しませんでした"), "{logs}");
}

#[tokio::test]
async fn a_could_not_verify_result_is_recorded_even_though_no_page_was_visited() {
    // D3: the record does not depend on a page load to happen — the shared
    // function records it inside `check` itself, so calling it once (as
    // startup does) is enough. Needs a real row to corrupt first.
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;
    corrupt_the_one_audit_row(&state).await;
    let _ = sui_id_core::audit_chain::check(&state.db, &state.clock, 100).await;
    let rows: i64 = state
        .db
        .with_conn(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = 'audit.chain.verification_failed'",
                [],
                |r| r.get(0),
            )
            .map_err(Into::into)
        })
        .await
        .expect("count");
    assert_eq!(rows, 1);
}
