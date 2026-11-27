//! RFC 149 Q3 (`0.95.0-q3-help-and-the-dashboard`), §6: the first-run
//! path, on-request help, the two new action items, and the one
//! language chain, through the real HTTP router.
//!
//! Tests 3 (help block length), 4 (`github_doc_url!` resolution), 9
//! (DNS class labels) and 11 (375px wrap) are source/unit scans beside
//! their own modules instead (G21): `pages::repair_checks` and
//! `tokens::tests` in `sui-id-web`, and
//! `handlers::admin::federation_providers::tests` /
//! `runtime::resolver::tests` in `sui-id`. Tests 1, 2, 5, 6, 7, 8 and
//! 10 of that dispatch's §6 table live here.

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, header};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::{FederationProviderId, UserId};
use sui_id_store::models::{FederationProviderRow, ProvisionMode, ValidationStatus};
use sui_id_store::repos::federation_provider;
use tower::ServiceExt;

/// Every console (admin-only) page that renders with a plain GET once
/// an admin is signed in — the set §1/§2 touch.
fn console_pages() -> &'static [&'static str] {
    &[
        "/admin",
        "/admin/users",
        "/admin/clients",
        "/admin/signing-keys",
        "/admin/federation-providers",
        "/admin/settings/basic",
        "/admin/settings/security",
        "/admin/settings/authentication",
        "/admin/settings/logs",
        "/admin/settings/other",
        "/admin/settings/email",
        "/admin/audit",
    ]
}

/// Every `/me` (account) page — any signed-in user, not just an admin.
fn account_pages() -> &'static [&'static str] {
    &[
        "/me/security/overview",
        "/me/security/mfa",
        "/me/security/passkeys",
        "/me/security/sessions",
        "/me/security/language",
    ]
}

async fn get(state: &AppState, uri: &str, session: &str) -> axum::response::Response {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET")
}

async fn get_anonymous(state: &AppState, uri: &str) -> axum::response::Response {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET")
}

async fn get_with_accept_language(
    state: &AppState,
    uri: &str,
    session: &str,
    accept_language: &str,
) -> axum::response::Response {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .header(header::ACCEPT_LANGUAGE, accept_language)
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

/// `html` from `<body` onward — the inline `<head>` stylesheet and
/// this file's own doc comments are not markup (same reasoning as
/// `q2_navigation.rs`'s `body_only`).
fn body_only(html: &str) -> &str {
    html.find("<body").map(|i| &html[i..]).unwrap_or(html)
}

/// `UPDATE users SET role = 'auditor'` directly — no HTTP path creates
/// an auditor in this suite (the same shape
/// `r096_b2_stage5_admin_surface.rs::demote_to_auditor` already uses).
async fn demote_to_auditor(state: &AppState, user_id: UserId) {
    state
        .db
        .with_conn(move |c| {
            Ok(c.execute(
                "UPDATE users SET role = 'auditor' WHERE id = ?1",
                [user_id.to_string()],
            )?)
        })
        .await
        .expect("demote to auditor");
}

fn requires_review_provider_row(slug: &str) -> FederationProviderRow {
    let now = chrono::Utc::now();
    FederationProviderRow {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: "Q3 Up".into(),
        issuer: format!("https://{slug}.example.com"),
        client_id: "client".into(),
        client_secret_enc: None,
        scopes: "openid email".into(),
        provision_mode: ProvisionMode::LinkOnly,
        enabled: false,
        allowed_origins: String::new(),
        config_version: 0,
        activation_generation: 0,
        validation_status: ValidationStatus::RequiresReview,
        evidence_config_version: None,
        evidence_activation_generation: None,
        evidence_policy_digest: None,
        evidence_observed_at: None,
        evidence_metadata_fingerprint: None,
        evidence_jwks_fingerprint: None,
        created_at: now,
        updated_at: now,
        id_token_algs: Some("RS256".into()),
    }
}

/// Insert a `RetainUntil` `master_key_rotation` row directly (test-only
/// — no CLI subprocess here), already past its own review date. Same
/// shape migration 0053's schema enforces, with `state = 'complete'`
/// (the only state a `RetainUntil` disposition is ever persisted
/// under in this suite).
async fn seed_kept_backup_overdue(
    state: &AppState,
    review_at: chrono::DateTime<chrono::Utc>,
    custody_reference: &str,
) {
    let now = chrono::Utc::now();
    let custody_reference = custody_reference.to_owned();
    state
        .db
        .with_conn(move |c| {
            Ok(c.execute(
                "INSERT INTO master_key_rotation \
                 (id, rotation_id, old_fingerprint, new_fingerprint, state, \
                  prepared_at, committed_at, completed_at, backup_filename, \
                  disposition_kind, review_at, custody_reference, verification_reference) \
                 VALUES (1, 'q3-test-rotation', 'old-fp', 'new-fp', 'complete', \
                  ?1, ?1, ?1, 'master.key.bak', 'retain_until', ?2, ?3, NULL)",
                rusqlite::params![now.to_rfc3339(), review_at.to_rfc3339(), custody_reference],
            )?)
        })
        .await
        .expect("seed kept-backup-overdue row");
}

/// Simulates `sui-id admin master-key backup renew`: moves
/// `review_at` into the future. Direct SQL (no CLI subprocess), same
/// row the dashboard's own read sees on the very next request.
async fn renew_kept_backup(state: &AppState) {
    let future = chrono::Utc::now() + chrono::Duration::days(365);
    state
        .db
        .with_conn(move |c| {
            Ok(c.execute(
                "UPDATE master_key_rotation SET review_at = ?1 WHERE id = 1",
                rusqlite::params![future.to_rfc3339()],
            )?)
        })
        .await
        .expect("renew kept backup");
}

// ---------- test 1: no hover-only tooltip anywhere ----------

#[tokio::test]
async fn no_hover_only_tooltip_anywhere() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    for uri in console_pages() {
        let html = html_of(get(&state, uri, &session).await).await;
        assert!(
            !body_only(&html).contains(" title=\""),
            "{uri}: has a title= attribute"
        );
    }
    for uri in account_pages() {
        let html = html_of(get(&state, uri, &session).await).await;
        assert!(
            !body_only(&html).contains(" title=\""),
            "{uri}: has a title= attribute"
        );
    }

    // Signed-out sign-in page.
    let signed_out = html_of(get_anonymous(&state, "/admin/login").await).await;
    assert!(
        !body_only(&signed_out).contains(" title=\""),
        "sign-in page has a title= attribute"
    );

    // A fresh instance's setup wizard (not yet initialized).
    let fresh = test_app();
    let setup_html = html_of(get_anonymous(&fresh, "/setup/admin").await).await;
    assert!(
        !body_only(&setup_html).contains(" title=\""),
        "setup wizard has a title= attribute"
    );

    // A POST-rendered page: create_client's own 200 re-render (the
    // new client + one-time secret shown back on the clients page).
    let (_id, _secret) = create_client(&state, &session).await;
    let post_rendered = html_of(get(&state, "/admin/clients", &session).await).await;
    assert!(
        !body_only(&post_rendered).contains(" title=\""),
        "POST-rendered clients page has a title= attribute"
    );
}

// ---------- test 2: every page states its purpose ----------

fn lede_text<'a>(html: &'a str, uri: &str) -> &'a str {
    let marker = r#"class="page-header__lede""#;
    let start = html
        .find(marker)
        .unwrap_or_else(|| panic!("{uri}: no page-header__lede"));
    let after_tag = html[start..]
        .find('>')
        .map(|i| start + i + 1)
        .unwrap_or_else(|| panic!("{uri}: malformed page-header__lede tag"));
    let end = html[after_tag..]
        .find("</p>")
        .map(|i| after_tag + i)
        .unwrap_or(html.len());
    &html[after_tag..end]
}

#[tokio::test]
async fn every_console_and_account_page_has_a_nonempty_lede() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    for uri in console_pages().iter().chain(account_pages()) {
        let html = html_of(get(&state, uri, &session).await).await;
        let text = lede_text(&html, uri);
        assert!(!text.trim().is_empty(), "{uri}: page-header__lede is empty");
    }
}

// ---------- test 5 & 6: the first-run path's steps, folding, and the
// SMTP-prompt de-duplication ----------

#[tokio::test]
async fn the_first_run_path_opens_with_recommended_and_optional_steps_and_folds() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let ja = sui_id_i18n::Locale::Ja.strings();

    // Fresh admin: steps 1-3 open (no "Done" badge on any of them,
    // since none is done yet), step 4 always "Recommended", step 5
    // "Optional" (never "Done" for a brand-new instance). The standalone
    // SMTP action item is suppressed — the path already names step 2.
    let html = html_of(get(&state, "/admin", &session).await).await;
    let body = body_only(&html);
    assert!(
        body.contains(ja.dashboard_gs_title),
        "fresh admin: path should be open and titled '{}'",
        ja.dashboard_gs_title
    );
    assert!(
        !body.contains(ja.dashboard_gs_done),
        "fresh admin: must not be folded yet"
    );
    assert!(
        body.contains(ja.dashboard_gs_status_recommended),
        "step 4 must always show Recommended"
    );
    assert!(
        body.contains(ja.dashboard_gs_status_optional),
        "step 5 (no federation provider yet) must show Optional, never Done"
    );
    assert!(
        !body.contains(ja.dashboard_warn_smtp),
        "the standalone SMTP item must be suppressed while the path's own step 2 says so"
    );

    // Step 1 (MFA): done.
    let (_secret, _codes) = enroll_mfa_for(&state, &session).await;
    // Step 2 (SMTP): done.
    enable_smtp(&state).await;
    // Step 3 (first app): done.
    let _ = create_client(&state, &session).await;

    let html = html_of(get(&state, "/admin", &session).await).await;
    let body = body_only(&html);
    assert!(
        body.contains(ja.dashboard_gs_done),
        "once steps 1-3 are all done, the path must fold to one line"
    );
    // `dashboard_gs_title` ("はじめに") is itself a prefix of
    // `dashboard_gs_done` ("はじめに: 完了"), so a plain substring
    // check can't distinguish them — read the `<summary>` element's
    // own text instead.
    let summary_start = body.find(r#"class="callout__title""#).expect("summary tag");
    let summary_text_start = body[summary_start..]
        .find('>')
        .map(|i| summary_start + i + 1)
        .expect("summary tag close");
    let summary_text_end = body[summary_text_start..]
        .find("</summary>")
        .map(|i| summary_text_start + i)
        .expect("summary close tag");
    let summary_text = body[summary_text_start..summary_text_end].trim();
    assert_eq!(
        summary_text, ja.dashboard_gs_done,
        "a folded path's <summary> must show only the done title, not the open one"
    );
    // Folded or not, step 4 never gets a "done" state — it is always
    // paired with "Recommended", never absent, never "Done".
    assert!(
        body.contains(ja.dashboard_gs_step4) && body.contains(ja.dashboard_gs_status_recommended),
        "step 4 must still show, and still say Recommended, even once steps 1-3 are done"
    );
}

#[tokio::test]
async fn auditor_sees_no_path_but_still_sees_the_smtp_prompt() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");
    demote_to_auditor(&state, admin.id).await;

    let html = html_of(get(&state, "/admin", &session).await).await;
    let body = body_only(&html);
    let ja = sui_id_i18n::Locale::Ja.strings();
    assert!(
        !body.contains(ja.dashboard_gs_title) && !body.contains(ja.dashboard_gs_done),
        "an auditor must never see the first-run path"
    );
    // An auditor is never "the administrator who hasn't done step 2",
    // so the path's suppression rule does not apply — they see the
    // standalone SMTP prompt on its own.
    assert!(
        body.contains(ja.dashboard_warn_smtp),
        "an auditor must see the standalone SMTP prompt (no path to de-duplicate against)"
    );
}

// ---------- test 7: the kept backup ----------

#[tokio::test]
async fn kept_backup_overdue_shows_date_and_custody_and_disappears_without_a_restart() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");
    // An auditor: literally "no path" shown, isolating the backup
    // item's own assertions from the first-run path's markup.
    demote_to_auditor(&state, admin.id).await;

    let review_at = chrono::Utc::now() - chrono::Duration::days(3);
    seed_kept_backup_overdue(&state, review_at, "safe-deposit-box-42").await;

    let html = html_of(get(&state, "/admin", &session).await).await;
    let body = body_only(&html);
    let ja = sui_id_i18n::Locale::Ja.strings();
    assert!(
        !body.contains(ja.dashboard_gs_title) && !body.contains(ja.dashboard_gs_done),
        "no path for an auditor"
    );
    assert!(
        body.contains("safe-deposit-box-42"),
        "the custody reference must appear"
    );
    // Same format `pages::common::fmt_time` uses ("%Y-%m-%d %H:%M UTC"),
    // reproduced here rather than imported (it is `pub(super)` in
    // `sui-id-web`, not part of its public surface).
    let formatted_review_at = review_at.format("%Y-%m-%d %H:%M UTC").to_string();
    assert!(
        body.contains(&(ja.dashboard_warn_backup_overdue)(
            &formatted_review_at,
            "safe-deposit-box-42",
        )),
        "the review-date sentence must appear in full"
    );
    assert!(
        !body.contains("master.key.bak"),
        "no filesystem path — just the date and the custody reference"
    );

    renew_kept_backup(&state).await;
    let html_after = html_of(get(&state, "/admin", &session).await).await;
    assert!(
        !html_after.contains("safe-deposit-box-42"),
        "renewed past now, the item must disappear on the very next request — no restart"
    );
}

// ---------- test 8: federation providers requiring review ----------

#[tokio::test]
async fn federation_providers_requiring_review_count_appears() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    federation_provider::create(
        &state.db,
        &requires_review_provider_row("q3-review-1"),
        None,
    )
    .await
    .expect("seed provider 1");
    federation_provider::create(
        &state.db,
        &requires_review_provider_row("q3-review-2"),
        None,
    )
    .await
    .expect("seed provider 2");

    let html = html_of(get(&state, "/admin", &session).await).await;
    let ja = sui_id_i18n::Locale::Ja.strings();
    assert!(
        body_only(&html).contains(&(ja.dashboard_warn_federation_requires_review)(2)),
        "the dashboard must show the exact count sentence for 2 providers"
    );
}

// ---------- test 10: one language chain ----------

#[tokio::test]
async fn console_and_account_pages_share_one_locale_chain() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");

    // No per-user preference, Accept-Language: en, server default is
    // Japanese (the shipped default) — both a console page and a /me
    // page must render English.
    let console_en =
        html_of(get_with_accept_language(&state, "/admin", &session, "en").await).await;
    assert!(
        console_en.contains("lang=\"en\""),
        "console page ignored Accept-Language: en"
    );
    let me_en =
        html_of(get_with_accept_language(&state, "/me/security/overview", &session, "en").await)
            .await;
    assert!(
        me_en.contains("lang=\"en\""),
        "/me page ignored Accept-Language: en"
    );

    // A saved per-user preference of Japanese overrides Accept-Language
    // on both — the same single chain, in the same priority order,
    // for both page families.
    sui_id_store::repos::users::set_preferred_lang(
        &state.db,
        admin.id,
        Some("ja"),
        chrono::Utc::now(),
    )
    .await
    .expect("set preferred_lang");

    let console_ja =
        html_of(get_with_accept_language(&state, "/admin", &session, "en").await).await;
    assert!(
        console_ja.contains("lang=\"ja\""),
        "console page must honour the saved preference over Accept-Language"
    );
    let me_ja =
        html_of(get_with_accept_language(&state, "/me/security/overview", &session, "en").await)
            .await;
    assert!(
        me_ja.contains("lang=\"ja\""),
        "/me page must honour the saved preference over Accept-Language"
    );
}

/// Q3's review: with one language chain, an English console reaches two
/// texts that were Japanese whatever the reader's language — the
/// dashboard's chart ranges (`SparklineRange::label_ja`, now removed)
/// and System → Email's connection-test result (hard-coded in the
/// handler). Both now follow the reader.
#[tokio::test]
async fn an_english_console_shows_no_japanese_only_text() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;

    let dashboard = html_of(get_with_accept_language(&state, "/admin", &session, "en").await).await;
    let body = body_only(&dashboard);
    assert!(
        body.contains("Last 24 hours"),
        "the range tabs follow the reader"
    );
    assert!(
        !body.contains("過去"),
        "an English dashboard must show no Japanese-only range label"
    );

    // SMTP is not configured on a fresh instance, so the test fails;
    // its message must be in the reader's language.
    let csrf = fetch_csrf(&state, &session).await;
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/settings/email/test")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::from(format!("_csrf={csrf}")))
                .expect("req"),
        )
        .await
        .expect("POST");
    let page = html_of(resp).await;
    let body = body_only(&page);
    assert!(
        body.contains("The SMTP connection test"),
        "the connection test's result follows the reader: {}",
        &body[..body.len().min(400)]
    );
    assert!(
        !body.contains("接続テスト"),
        "an English console must not show the Japanese test result"
    );
}
