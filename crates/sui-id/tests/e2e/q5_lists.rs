//! RFC 149 Q5 (`0.95.0-q5-the-lists`), §5: no list says less than it
//! is. §5's named tests 1, 2, 3, 4, 5, 6, 7 live here (test 8, the
//! client→app rename scan, is a unit test beside `pages/clients.rs`
//! instead — G21, no HTTP round trip needed for it).

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::UserId;
use sui_id_store::models::{
    ClientRow, ConsentPolicy, RegistrationSource, Role, UserRow, UserSource,
};
use tower::ServiceExt;

async fn get(state: &AppState, uri: &str, session: &str) -> axum::response::Response {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET")
}

async fn html_of(resp: axum::response::Response) -> String {
    let body = read_body(resp.into_body()).await;
    String::from_utf8_lossy(&body).to_string()
}

fn seed_user_row(username: &str, created_at: chrono::DateTime<chrono::Utc>) -> UserRow {
    UserRow {
        id: UserId::new(),
        username: username.to_owned(),
        display_name: None,
        email: None,
        email_normalized: None,
        email_verified_at: None,
        preferred_lang: None,
        is_admin: false,
        role: Role::User,
        is_disabled: false,
        is_deleted: false,
        last_login_at: None,
        user_uuid: uuid::Uuid::new_v4(),
        created_at,
        updated_at: created_at,
        failed_login_count: 0,
        locked_until: None,
        source: UserSource::default(),
        external_stable_id: None,
    }
}

fn seed_client_row(name: &str, created_at: chrono::DateTime<chrono::Utc>) -> ClientRow {
    ClientRow {
        id: sui_id_shared::ids::ClientId::new(),
        name: name.to_owned(),
        confidential: false,
        secret_hash: None,
        redirect_uris: vec!["https://rp.test/cb".into()],
        allowed_scopes: "openid".into(),
        post_logout_redirect_uris: vec![],
        is_disabled: false,
        is_deleted: false,
        consent_policy: ConsentPolicy::default(),
        registered_via: RegistrationSource::Admin,
        logo_uri: None,
        homepage_uri: None,
        privacy_policy_uri: None,
        tos_uri: None,
        token_endpoint_auth_method: None,
        grant_types: None,
        response_types: None,
        created_at,
        updated_at: created_at,
    }
}

/// `<span class="code...">{text}</span>` is the users table's own
/// first cell (the username) and the audit table's own target cell
/// (`class="code cell-id-inline"`, Q5's own merge of that cell with
/// its copy button) -- collecting every one tells which rows a page
/// actually rendered, in order. Matches the `class="code` *prefix* so
/// either shape is caught.
fn coded_names(html: &str) -> Vec<String> {
    let marker = "<span class=\"code";
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = html[from..].find(marker) {
        let start = from + rel + marker.len();
        let Some(tag_end_rel) = html[start..].find('>') else {
            break;
        };
        let text_start = start + tag_end_rel + 1;
        let Some(end_rel) = html[text_start..].find("</span>") else {
            break;
        };
        out.push(html[text_start..text_start + end_rel].to_owned());
        from = text_start + end_rel;
    }
    out
}

/// The clients table's own first cell is the app's name in a plain
/// (uncoded) `<span>`, immediately inside that cell's `row gap-1` div
/// -- unlike the users table, whose first cell *is* `class="code"`
/// (the username), so this needs its own marker.
fn app_names(html: &str) -> Vec<String> {
    let marker = "<div class=\"row gap-1\"><span>";
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = html[from..].find(marker) {
        let start = from + rel + marker.len();
        let Some(end_rel) = html[start..].find("</span>") else {
            break;
        };
        out.push(html[start..start + end_rel].to_owned());
        from = start + end_rel;
    }
    out
}

// ---------- test 1: users page in exact ranges, every user once ----------

#[tokio::test]
async fn users_page_shows_exact_ranges_and_every_user_appears_once() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // 119 more users (+ the admin from setup = 120 total). Half share
    // one `created_at` exactly, so the `id` tie-break is load-bearing,
    // not merely present.
    let base = chrono::Utc::now();
    let mut expected_usernames = std::collections::BTreeSet::new();
    expected_usernames.insert(USERNAME.to_owned());
    // (created_at, id, username) of every seeded user, to state the
    // order the pages must follow: `created_at`, then `id`.
    let mut seeded: Vec<(chrono::DateTime<chrono::Utc>, String, String)> = Vec::new();
    for i in 0..119 {
        let created_at = if i % 2 == 0 {
            base
        } else {
            base + chrono::Duration::seconds(i)
        };
        let name = format!("user{i:03}");
        let row = seed_user_row(&name, created_at);
        seeded.push((row.created_at, row.id.to_string(), name.clone()));
        sui_id_store::repos::users::create(&state.db, &row)
            .await
            .expect("seed user");
        expected_usernames.insert(name);
    }
    assert_eq!(
        sui_id_store::repos::users::count(&state.db)
            .await
            .expect("count"),
        120
    );

    let page1 = html_of(get(&state, "/admin/users?page=1", &session).await).await;
    assert!(
        page1.contains("Showing 1\u{2013}50 of 120."),
        "page 1 caption: {page1}"
    );
    let names1 = coded_names(&page1);
    assert_eq!(names1.len(), 50, "page 1 row count");

    let page3 = html_of(get(&state, "/admin/users?page=3", &session).await).await;
    assert!(
        page3.contains("Showing 101\u{2013}120 of 120."),
        "page 3 caption: {page3}"
    );
    let names3 = coded_names(&page3);
    assert_eq!(names3.len(), 20, "page 3 row count");

    let page2 = html_of(get(&state, "/admin/users?page=2", &session).await).await;
    let names2 = coded_names(&page2);
    assert_eq!(names2.len(), 50, "page 2 row count");

    let mut seen = std::collections::BTreeSet::new();
    for n in names1.iter().chain(names2.iter()).chain(names3.iter()) {
        assert!(seen.insert(n.clone()), "{n} appeared on more than one page");
    }
    assert_eq!(
        seen, expected_usernames,
        "every user must appear exactly once"
    );

    // Q5's review: "every user once" holds for any stable order, so it
    // could not catch a dropped tie-break (SQLite returns ties in
    // insertion order). The pages must follow `created_at`, then `id`:
    // with ties, insertion order and `id` order differ, so this fails
    // if the tie-break goes.
    seeded.sort();
    let expected_order: Vec<String> = std::iter::once(USERNAME.to_owned())
        .chain(seeded.into_iter().map(|(_, _, name)| name))
        .collect();
    let served: Vec<String> = names1.into_iter().chain(names2).chain(names3).collect();
    assert_eq!(
        served, expected_order,
        "the pages follow created_at, then id"
    );
}

// ---------- test 2: the same for apps ----------

#[tokio::test]
async fn apps_page_shows_exact_ranges_and_every_app_appears_once() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    let base = chrono::Utc::now();
    let mut expected_names = std::collections::BTreeSet::new();
    for i in 0..120 {
        let created_at = if i % 2 == 0 {
            base
        } else {
            base + chrono::Duration::seconds(i)
        };
        let name = format!("app{i:03}");
        let row = seed_client_row(&name, created_at);
        sui_id_store::repos::clients::create(&state.db, &row)
            .await
            .expect("seed app");
        expected_names.insert(name);
    }
    assert_eq!(
        sui_id_store::repos::clients::count(&state.db)
            .await
            .expect("count"),
        120
    );

    let page1 = html_of(get(&state, "/admin/clients?page=1", &session).await).await;
    assert!(
        page1.contains("Showing 1\u{2013}50 of 120."),
        "page 1 caption: {page1}"
    );
    let page3 = html_of(get(&state, "/admin/clients?page=3", &session).await).await;
    assert!(
        page3.contains("Showing 101\u{2013}120 of 120."),
        "page 3 caption: {page3}"
    );
    let page2 = html_of(get(&state, "/admin/clients?page=2", &session).await).await;

    let names1 = app_names(&page1);
    let names2 = app_names(&page2);
    let names3 = app_names(&page3);
    assert_eq!(names1.len(), 50);
    assert_eq!(names2.len(), 50);
    assert_eq!(names3.len(), 20);

    let mut seen = std::collections::BTreeSet::new();
    for n in names1.iter().chain(names2.iter()).chain(names3.iter()) {
        if expected_names.contains(n) {
            assert!(seen.insert(n.clone()), "{n} appeared on more than one page");
        }
    }
    assert_eq!(seen, expected_names, "every app must appear exactly once");
}

// ---------- test 3: the audit caption counts matching entries ----------

#[tokio::test]
async fn audit_caption_counts_matching_entries_not_rendered_rows() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // 300 matching rows (a dedicated action name so the count is
    // exact and unaffected by setup's own audit rows).
    for i in 0..300 {
        let row = sui_id_store::models::AuditLogRow {
            at: chrono::Utc::now(),
            actor: None,
            action: "q5.test_event".into(),
            target: Some(format!("row-{i}")),
            result: "ok".into(),
            note: None,
        };
        sui_id_store::repos::audit::append(&state.db, &row)
            .await
            .expect("seed audit row");
    }

    let html = html_of(get(&state, "/admin/audit?q=q5.test_event", &session).await).await;
    assert!(
        html.contains("300 matching entries"),
        "caption must count every match, not the 50 shown: {html}"
    );
    let rows_shown = html.matches("q5.test_event").count();
    // One per rendered row's action cell, plus the filter's own
    // `value="q5.test_event"` on the input -- either way, far short
    // of 300, confirming the page itself still only rendered one page.
    assert!(
        rows_shown < 300,
        "the page itself must still be one page: {rows_shown}"
    );
}

// ---------- test 4: seq paging is stable under concurrent writes ----------

#[tokio::test]
async fn seq_paging_continues_exactly_where_the_previous_page_ended() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    for i in 0..80 {
        let row = sui_id_store::models::AuditLogRow {
            at: chrono::Utc::now(),
            actor: None,
            action: "q5.stable".into(),
            target: Some(format!("a-{i}")),
            result: "ok".into(),
            note: None,
        };
        sui_id_store::repos::audit::append(&state.db, &row)
            .await
            .expect("seed");
    }

    let page1_html = html_of(get(&state, "/admin/audit?q=q5.stable", &session).await).await;
    let page1_targets: Vec<String> = coded_names(&page1_html)
        .into_iter()
        .filter(|s| s.starts_with("a-"))
        .collect();
    assert_eq!(page1_targets.len(), 50);

    // New events arrive between page reads.
    for i in 0..5 {
        let row = sui_id_store::models::AuditLogRow {
            at: chrono::Utc::now(),
            actor: None,
            action: "q5.stable".into(),
            target: Some(format!("new-{i}")),
            result: "ok".into(),
            note: None,
        };
        sui_id_store::repos::audit::append(&state.db, &row)
            .await
            .expect("seed new");
    }

    // Follow the page's own "older" link rather than guessing a seq.
    // `&` renders HTML-escaped (`&amp;`) in the attribute, as any real
    // browser's address bar would decode before the next navigation.
    let older_marker = "href=\"";
    let older_prefix = "/admin/audit?q=q5.stable&amp;before=";
    let older_href = {
        let mut from = 0usize;
        let mut found = None;
        while let Some(rel) = page1_html[from..].find(older_marker) {
            let start = from + rel + older_marker.len();
            let end = start + page1_html[start..].find('"').expect("quote");
            let href = &page1_html[start..end];
            if href.starts_with(older_prefix) {
                found = Some(href.replace("&amp;", "&"));
                break;
            }
            from = end;
        }
        found.expect("an older-page link must exist")
    };

    let page2_html = html_of(get(&state, &older_href, &session).await).await;
    let page2_targets: Vec<String> = coded_names(&page2_html)
        .into_iter()
        .filter(|s| s.starts_with("a-"))
        .collect();
    assert_eq!(
        page2_targets.len(),
        30,
        "the remaining 30 of the original 80"
    );

    let mut seen = std::collections::HashSet::new();
    for t in page1_targets.iter().chain(page2_targets.iter()) {
        assert!(seen.insert(t.clone()), "{t} repeated across pages");
    }
    for i in 0..80 {
        assert!(
            seen.contains(&format!("a-{i}")),
            "a-{i} skipped across pages"
        );
    }
}

// ---------- test 5: the filter survives paging ----------

#[tokio::test]
async fn the_filter_survives_paging_to_an_older_page() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    for i in 0..60 {
        for action in ["q5.filtered_a", "q5.filtered_b"] {
            let row = sui_id_store::models::AuditLogRow {
                at: chrono::Utc::now(),
                actor: None,
                action: action.into(),
                target: Some(format!("{action}-{i}")),
                result: "ok".into(),
                note: None,
            };
            sui_id_store::repos::audit::append(&state.db, &row)
                .await
                .expect("seed");
        }
    }

    let page1_html = html_of(get(&state, "/admin/audit?q=q5.filtered_a", &session).await).await;
    assert!(
        !page1_html.contains("q5.filtered_b"),
        "sanity: page 1 should not show the other action"
    );
    let older_prefix = "/admin/audit?q=q5.filtered_a&amp;before=";
    let older_href = {
        let marker = "href=\"";
        let mut from = 0usize;
        let mut found = None;
        while let Some(rel) = page1_html[from..].find(marker) {
            let start = from + rel + marker.len();
            let end = start + page1_html[start..].find('"').expect("quote");
            let href = &page1_html[start..end];
            if href.starts_with(older_prefix) {
                found = Some(href.replace("&amp;", "&"));
                break;
            }
            from = end;
        }
        found.expect("an older-page link, with the filter kept, must exist")
    };

    let page2_html = html_of(get(&state, &older_href, &session).await).await;
    assert!(
        !page2_html.contains("q5.filtered_b"),
        "the older page must still be filtered to q5.filtered_a only"
    );
    assert!(page2_html.contains("q5.filtered_a"));
}

// ---------- test 6: the CSV export is complete ----------

#[tokio::test]
async fn the_csv_export_includes_every_matching_row() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    for i in 0..2500 {
        let row = sui_id_store::models::AuditLogRow {
            at: chrono::Utc::now(),
            actor: None,
            action: "q5.csv_complete".into(),
            target: Some(format!("row-{i}")),
            result: "ok".into(),
            note: None,
        };
        sui_id_store::repos::audit::append(&state.db, &row)
            .await
            .expect("seed");
    }

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/audit.csv?q=q5.csv_complete")
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("csv");
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = read_body(resp.into_body()).await;
    let body = String::from_utf8_lossy(&bytes);
    // The header row first (no comment line before it), then one line
    // per row.
    assert!(
        body.starts_with("when,actor,action,target,result,note\n"),
        "the header row is the first line"
    );
    let data_lines = body.lines().count() - 1;
    assert_eq!(data_lines, 2500, "the old 2000-row cap must not reappear");
    assert_eq!(body.matches("\"row-").count(), 2500);
}

// ---------- test 7: excerpts say so ----------

#[tokio::test]
async fn dashboard_recent_events_say_they_are_an_excerpt_and_link_to_the_audit_log() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    // At least one "important" event beyond setup's own, so the
    // excerpt has something to caption.
    sui_id_store::repos::audit::append(
        &state.db,
        &sui_id_store::models::AuditLogRow {
            at: chrono::Utc::now(),
            actor: None,
            action: "client.create".into(),
            target: None,
            result: "ok".into(),
            note: None,
        },
    )
    .await
    .expect("seed");

    let html = html_of(get(&state, "/admin", &session).await).await;
    assert!(
        html.contains("Showing the newest"),
        "the dashboard's recent-events card must say it is an excerpt: {html}"
    );
    assert!(
        html.contains("href=\"/admin/audit\"") && html.contains("View all"),
        "and link to the full audit log: {html}"
    );
}
