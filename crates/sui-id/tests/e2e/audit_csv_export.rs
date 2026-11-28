//! The audit log's CSV export cannot be forged, or turned into
//! spreadsheet formulas, by text an unauthenticated visitor chose.
//!
//! A failed sign-in records the submitted username as the audit row's
//! target. Until `0.94.1`, the export wrote a `"` inside a value as
//! three quotes, which broke the record and could forge columns or rows,
//! and wrote a value starting with `=` as is, which a spreadsheet
//! evaluates as a formula (CWE-1236).

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, header};
use sui_id::build_router;
use tower::ServiceExt;

/// Split one CSV record into its fields, as RFC 4180 reads it.
fn rfc4180_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    let mut quoted = false;
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            (true, '"') => quoted = false,
            (false, '"') => quoted = true,
            (false, ',') => fields.push(std::mem::take(&mut cur)),
            (_, c) => cur.push(c),
        }
    }
    fields.push(cur);
    fields
}

#[tokio::test]
async fn the_csv_export_cannot_be_forged_or_turned_into_formulas_by_a_username() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let crafted = [
        "=HYPERLINK(\"https://evil.example\",\"x\")",
        "a\",\"forged\",\"row",
        "+1",
        "@cmd",
    ];
    for name in crafted {
        let resp = build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/admin/login")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body(Body::from(format!(
                        "username={}&password=wrong-password-123",
                        url::form_urlencoded::byte_serialize(name.as_bytes()).collect::<String>()
                    )))
                    .expect("req"),
            )
            .await
            .expect("login");
        assert!(
            !resp.status().is_server_error(),
            "{name}: {}",
            resp.status()
        );
    }
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/audit.csv")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("csv");
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes).to_string();
    for line in body.lines() {
        assert_eq!(
            rfc4180_fields(line).len(),
            6,
            "every record has six fields: {line}"
        );
    }
    let targets: Vec<String> = body
        .lines()
        .skip(1)
        .map(|l| rfc4180_fields(l)[3].clone())
        .collect();
    for name in crafted {
        let expected = if name.starts_with(['=', '+', '-', '@']) {
            format!("'{name}")
        } else {
            name.to_owned()
        };
        assert!(
            targets.contains(&expected),
            "{name:?} must read back as {expected:?}, intact and inert; targets: {targets:?}"
        );
    }
}
