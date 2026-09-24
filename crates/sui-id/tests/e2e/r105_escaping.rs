//! RFC 105 — audit-note escaping, end to end. An administrator's free-text
//! reason cannot introduce a pair into the note, and the CSV export shows the
//! stored (encoded) form exactly. (The admin audit page renders no note.)

use super::common::*;
use super::r103_stage1::{Resp, get, send};
use super::r103_stage3::{Admin, admin, target_user};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id_shared::ids::UserId;

// A `step_up` value the real one can never be (a real one is seconds since a
// step-up, well under a minute in a test), so its absence is unambiguous.
const FORGED: &str = "x step_up=fresh:totp:777777 via=cli invalidated=9";
const FORGED_ENCODED: &str = "x%20step_up%3Dfresh:totp:777777%20via%3Dcli%20invalidated%3D9";

async fn issue_with_reason(a: &Admin, id: UserId, reason: &str) -> Resp {
    let csrf = fetch_csrf(&a.state, &a.session).await;
    send(
        &a.state,
        Request::builder()
            .method(Method::POST)
            .uri(format!("/admin/users/{id}/recovery-link"))
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(
                header::COOKIE,
                format!("sui_id_session={}; sui_id_csrf={csrf}", a.session),
            )
            .body(Body::from(format!(
                "_csrf={csrf}&reason={}&_confirmed=1",
                urlencode(reason)
            )))
            .expect("req"),
    )
    .await
}

async fn as_admin(a: &Admin, uri: &str) -> Resp {
    send(
        &a.state,
        Request::builder()
            .method(Method::GET)
            .uri(uri)
            .header(header::COOKIE, format!("sui_id_session={}", a.session))
            .body(Body::empty())
            .expect("req"),
    )
    .await
}

async fn note_of_last_issue(a: &Admin) -> String {
    a.state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT note FROM audit_log WHERE action = 'user.recovery_link.issued' \
                 ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get::<_, String>(0),
            )?)
        })
        .await
        .expect("note")
}

#[tokio::test]
async fn r105_a_forged_reason_is_stored_as_one_encoded_value() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    let r = issue_with_reason(&a, bob, FORGED).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.body);

    let note = note_of_last_issue(&a).await;
    assert!(
        note.starts_with(&format!("reason={FORGED_ENCODED} via=web expires_at=")),
        "{note}"
    );
    // The imitations are not there as pairs, or as substrings a query could hit.
    for forged in ["step_up=fresh:totp:777777", "via=cli", "invalidated=9"] {
        assert!(!note.contains(forged), "{forged} in {note}");
    }
    // Exactly one of each real key.
    for key in ["via=", "expires_at=", "invalidated=", "step_up="] {
        assert_eq!(note.matches(key).count(), 1, "{key} in {note}");
    }
}

#[tokio::test]
async fn r105_the_csv_shows_the_stored_form_exactly_and_the_audit_page_shows_no_note() {
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    assert_eq!(
        issue_with_reason(&a, bob, FORGED).await.status,
        StatusCode::OK
    );

    let csv = as_admin(&a, "/admin/audit.csv").await;
    assert_eq!(csv.status, StatusCode::OK);
    assert!(
        csv.body
            .contains(&format!("reason={FORGED_ENCODED} via=web")),
        "the CSV carries the encoded note as stored"
    );
    assert!(!csv.body.contains("step_up=fresh:totp:777777 via=cli"));

    // The admin audit page has no note column at all (it shows time, actor,
    // action, target and outcome), so there is nothing on it to mislead; it
    // must still render, and must not carry the forged text either.
    let page = as_admin(&a, "/admin/audit").await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(!page.body.contains("step_up=fresh:totp:777777"));
}

#[tokio::test]
async fn r105_an_ordinary_event_is_unchanged() {
    // A value with no space, `=`, `%` or control character is written as it
    // always was: the RFC 103 pinned note formats do not move.
    let a = admin().await;
    let bob = target_user(&a, "bob").await;
    assert_eq!(
        issue_with_reason(&a, bob, "ticket-4711").await.status,
        StatusCode::OK
    );
    let note = note_of_last_issue(&a).await;
    assert!(
        note.starts_with("reason=ticket-4711 via=web expires_at="),
        "{note}"
    );
    let _ = get(&a.state, "/admin/login").await;
}
