//! RFC 115 stage 2 — creation without a password, end to end (D2, D3, D4, D9,
//! D10).
//!
//! `/admin/users/new` sets no password; creation leads to the recovery-link
//! confirm screen; the holder chooses the password through the link; the
//! policy and the breach check run there; a second administrator can be
//! created and activated on the web while a live one still cannot be reached;
//! and the writers of `credentials` are pinned to an allowlist so a fifth
//! fails CI rather than review.

use super::common::*;
use super::r102_stage1::sign_in;
use super::r103_stage1::{NEW_PASSWORD, Resp, csrf_from, events, exec, get, send};
use super::r103_stage3::{Admin, admin, stepped_up};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::AppState;
use sui_id_shared::ids::UserId;

const REASON: &str = "caller verified by call-back, ticket 4711";

async fn scalar(state: &AppState, sql: String) -> i64 {
    state
        .db
        .with_conn(move |c| Ok(c.query_row(&sql, [], |r| r.get(0))?))
        .await
        .expect("scalar")
}

async fn credential_rows(state: &AppState, user: UserId) -> i64 {
    scalar(
        state,
        format!("SELECT COUNT(*) FROM credentials WHERE user_id = '{user}'"),
    )
    .await
}

/// `POST /admin/users` as the signed-in administrator. `extra` is appended to
/// the form body verbatim, so a test can submit a field the form no longer has.
async fn web_create(a: &Admin, username: &str, email: &str, is_admin: bool, extra: &str) -> Resp {
    let csrf = fetch_csrf(&a.state, &a.session).await;
    let mut body = format!(
        "_csrf={csrf}&username={}&display_name=&email={}",
        urlencode(username),
        urlencode(email)
    );
    if is_admin {
        body.push_str("&is_admin=true");
    }
    body.push_str(extra);
    send(
        &a.state,
        Request::builder()
            .method(Method::POST)
            .uri("/admin/users")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(
                header::COOKIE,
                format!("sui_id_session={}; sui_id_csrf={csrf}", a.session),
            )
            .body(Body::from(body))
            .expect("req"),
    )
    .await
}

fn created_id(r: &Resp) -> UserId {
    let loc = r.location.as_deref().expect("a redirect");
    let id = loc
        .strip_prefix("/admin/users/")
        .and_then(|rest| rest.strip_suffix("/recovery-link-confirm"))
        .unwrap_or_else(|| panic!("redirect to the confirm screen, got {loc}"));
    id.parse().expect("user id")
}

async fn get_as(a: &Admin, uri: &str) -> Resp {
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

async fn post_issue(a: &Admin, id: UserId) -> Resp {
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
                urlencode(REASON)
            )))
            .expect("req"),
    )
    .await
}

fn token_on_page(body: &str) -> String {
    let start = body
        .find("/reset-password#t=")
        .expect("the link is on the page")
        + "/reset-password#t=".len();
    let end = body[start..]
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .map(|i| start + i)
        .expect("the link ends");
    body[start..end].to_owned()
}

/// Complete a reset with an explicit new password.
async fn complete_with(state: &AppState, token: &str, password: &str) -> Resp {
    let csrf = csrf_from(state, "/reset-password").await;
    send(
        state,
        Request::builder()
            .method(Method::POST)
            .uri("/reset-password")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::COOKIE, format!("sui_id_csrf={csrf}"))
            .body(Body::from(format!(
                "_csrf={csrf}&token={}&password={pw}&confirm_password={pw}",
                urlencode(token),
                pw = urlencode(password)
            )))
            .expect("req"),
    )
    .await
}

/// A password sign-in attempt; the status only.
async fn login_status(state: &AppState, username: &str, password: &str) -> StatusCode {
    send(
        state,
        Request::builder()
            .method(Method::POST)
            .uri("/admin/login")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "username={}&password={}&next=",
                urlencode(username),
                urlencode(password)
            )))
            .expect("req"),
    )
    .await
    .status
}

/// Issue the link for `id` from the confirm screen the redirect leads to, and
/// return the token. Asserts each step on the way.
async fn issue_from_confirm(a: &Admin, id: UserId) -> String {
    let confirm = get_as(a, &format!("/admin/users/{id}/recovery-link-confirm")).await;
    assert_eq!(confirm.status, StatusCode::OK, "{}", confirm.body);
    let issued = post_issue(a, id).await;
    assert_eq!(issued.status, StatusCode::OK, "{}", issued.body);
    token_on_page(&issued.body)
}

// ── D2, D3, D4 ────────────────────────────────────────────────────────

#[tokio::test]
async fn r115_s2_creating_a_user_on_the_web_sets_no_password_and_leads_to_issuance() {
    let a = admin().await;

    // The form has no password field.
    let form = get_as(&a, "/admin/users/new").await;
    assert_eq!(form.status, StatusCode::OK);
    assert!(
        form.body.contains(r#"name="username""#),
        "the form rendered"
    );
    // (`type="password"` alone is not a test: the page's inline stylesheet
    // names that selector for every form.)
    assert!(
        !form.body.contains(r#"name="password""#) && !form.body.contains(r#"id="u-pw""#),
        "the create-user form must not ask for a password"
    );

    // A password submitted anyway (an old form, a script) is ignored, not stored.
    let r = web_create(
        &a,
        "dave",
        "dave@test.invalid",
        false,
        "&password=admin-chosen-password-1",
    )
    .await;
    assert!(r.status.is_redirection(), "{} {}", r.status, r.body);
    let dave = created_id(&r);

    assert_eq!(
        credential_rows(&a.state, dave).await,
        0,
        "no credential row"
    );
    assert_eq!(
        login_status(&a.state, "dave", "admin-chosen-password-1").await,
        StatusCode::UNAUTHORIZED,
        "the password the administrator typed does not work"
    );
    assert_eq!(events(&a.state, "user.create").await, 1);
    assert_eq!(
        events(&a.state, "user.create_warned_hibp").await,
        0,
        "the retired event is never written"
    );
    // Nothing is issued by creation itself: two commands, not one.
    assert_eq!(
        scalar(
            &a.state,
            format!("SELECT COUNT(*) FROM password_reset_tokens WHERE user_id = '{dave}'")
        )
        .await,
        0
    );
}

#[tokio::test]
async fn r115_s2_a_created_user_activates_through_the_issued_link_with_their_own_password() {
    let a = admin().await;
    let r = web_create(&a, "dave", "dave@test.invalid", false, "").await;
    let dave = created_id(&r);

    let token = issue_from_confirm(&a, dave).await;
    let done = complete_with(&a.state, &token, NEW_PASSWORD).await;
    assert!(done.status.is_redirection(), "{}", done.status);
    assert_eq!(done.location.as_deref(), Some("/admin/login?reset=ok"));

    assert_eq!(credential_rows(&a.state, dave).await, 1);
    let session = sign_in(&a.state, "dave", NEW_PASSWORD).await;
    assert!(
        !session.is_empty(),
        "dave signs in with the password he chose"
    );

    assert_eq!(events(&a.state, "user.create").await, 1);
    assert_eq!(events(&a.state, "user.recovery_link.issued").await, 1);
    assert_eq!(
        scalar(
            &a.state,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.password.reset_completed' \
             AND note = 'origin=web'"
                .into()
        )
        .await,
        1,
        "the completion is joined to the web issuance"
    );
}

// ── the policy and the breach check run where the holder chooses ───────

/// The setup administrator with a second factor and a fresh step-up, over an
/// app whose breach client the test controls.
async fn admin_with_hibp() -> (
    Admin,
    std::sync::Arc<sui_id_core::hibp::test_support::InMemoryHibpClient>,
) {
    let (state, _mailer, hibp) = test_app_with_hibp();
    let session = complete_setup_and_login(&state).await;
    let id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin")
        .id;
    exec(
        &state,
        format!(
            "INSERT INTO user_webauthn_credentials \
             (id, user_id, credential_id, passkey_enc, nickname, created_at) \
             VALUES ('{}', '{id}', X'0102', X'00', 'k', '2026-01-01T00:00:00Z')",
            uuid::Uuid::new_v4()
        ),
    )
    .await;
    stepped_up(&state, &session).await;
    (Admin { state, id, session }, hibp)
}

#[tokio::test]
async fn r115_s2_the_password_policy_and_the_breach_check_run_at_activation() {
    let (a, hibp) = admin_with_hibp().await;
    let dave = created_id(&web_create(&a, "dave", "dave@test.invalid", false, "").await);
    let token = issue_from_confirm(&a, dave).await;

    // Policy: too short is refused, the link survives, no credential appears.
    let short = complete_with(&a.state, &token, "short").await;
    assert_eq!(short.status, StatusCode::BAD_REQUEST);
    assert_eq!(credential_rows(&a.state, dave).await, 0);

    // Block mode: a breached password is refused, the link survives.
    set_hibp_mode(&a.state, sui_id_store::models::HibpMode::Block).await;
    hibp.set_breached(NEW_PASSWORD, 9001);
    let blocked = complete_with(&a.state, &token, NEW_PASSWORD).await;
    assert_eq!(blocked.status, StatusCode::BAD_REQUEST);
    assert_eq!(credential_rows(&a.state, dave).await, 0);

    // Warn mode (D9): the breached password is allowed, and the completion
    // event records that it was, which used to be discarded.
    set_hibp_mode(&a.state, sui_id_store::models::HibpMode::Warn).await;
    let allowed = complete_with(&a.state, &token, NEW_PASSWORD).await;
    assert!(allowed.status.is_redirection(), "{}", allowed.status);
    assert_eq!(credential_rows(&a.state, dave).await, 1);
    assert_eq!(
        scalar(
            &a.state,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.password.reset_completed' \
             AND note = 'origin=web hibp=warned'"
                .into()
        )
        .await,
        1,
        "the warn-mode outcome is on the event"
    );
}

#[tokio::test]
async fn r115_s2_a_clean_activation_records_no_breach_warning() {
    let (a, _hibp) = admin_with_hibp().await;
    let dave = created_id(&web_create(&a, "dave", "dave@test.invalid", false, "").await);
    let token = issue_from_confirm(&a, dave).await;
    assert!(
        complete_with(&a.state, &token, NEW_PASSWORD)
            .await
            .status
            .is_redirection()
    );
    assert_eq!(
        scalar(
            &a.state,
            "SELECT COUNT(*) FROM audit_log WHERE action = 'auth.password.reset_completed' \
             AND note = 'origin=web'"
                .into()
        )
        .await,
        1
    );
}

// ── D10: a second administrator ───────────────────────────────────────

#[tokio::test]
async fn r115_s2_a_second_administrator_is_created_and_activated_on_the_web() {
    let a = admin().await;
    let r = web_create(&a, "bea", "bea@test.invalid", true, "").await;
    let bea = created_id(&r);
    let role: i64 = scalar(
        &a.state,
        format!("SELECT COUNT(*) FROM users WHERE id = '{bea}' AND role = 'admin'"),
    )
    .await;
    assert_eq!(role, 1, "created as an administrator");

    // The button is offered for an administrator that has never been activated.
    let detail = get_as(&a, &format!("/admin/users/{bea}")).await;
    assert!(
        detail
            .body
            .contains(&format!("/admin/users/{bea}/recovery-link-confirm")),
        "the issue-link action is offered for a never-activated administrator"
    );

    let token = issue_from_confirm(&a, bea).await;
    let done = complete_with(&a.state, &token, NEW_PASSWORD).await;
    assert!(done.status.is_redirection(), "{}", done.status);
    let session = sign_in(&a.state, "bea", NEW_PASSWORD).await;
    assert!(!session.is_empty(), "the second administrator signs in");
}

#[tokio::test]
async fn r115_s2_a_live_administrator_is_still_refused_and_the_button_is_gone() {
    let a = admin().await;
    let bea = created_id(&web_create(&a, "bea", "bea@test.invalid", true, "").await);
    let token = issue_from_confirm(&a, bea).await;
    assert!(
        complete_with(&a.state, &token, NEW_PASSWORD)
            .await
            .status
            .is_redirection()
    );
    // Bea signs in once: she now has a credential *and* a last login.
    sign_in(&a.state, "bea", NEW_PASSWORD).await;
    assert_eq!(
        scalar(
            &a.state,
            format!("SELECT COUNT(*) FROM users WHERE id = '{bea}' AND last_login_at IS NOT NULL")
        )
        .await,
        1
    );

    // The first administrator, stepped up again, still cannot issue for her.
    stepped_up(&a.state, &a.session).await;
    let before = scalar(
        &a.state,
        "SELECT COUNT(*) FROM password_reset_tokens".into(),
    )
    .await;
    let refused = post_issue(&a, bea).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{}", refused.body);
    assert_eq!(
        scalar(
            &a.state,
            "SELECT COUNT(*) FROM password_reset_tokens".into()
        )
        .await,
        before,
        "a refusal writes no token"
    );
    let detail = get_as(&a, &format!("/admin/users/{bea}")).await;
    assert!(
        !detail
            .body
            .contains(&format!("/admin/users/{bea}/recovery-link-confirm")),
        "the issue-link action is hidden for a live administrator"
    );
}

#[tokio::test]
async fn r115_s2_an_account_with_a_credential_but_no_sign_in_is_not_reachable_either() {
    // The `last_login_at` half and the credential half of D10's predicate each
    // hold on their own (the store test pins them; this is the same rule
    // through the web).
    let a = admin().await;
    let bea = created_id(&web_create(&a, "bea", "bea@test.invalid", true, "").await);
    let token = issue_from_confirm(&a, bea).await;
    assert!(
        complete_with(&a.state, &token, NEW_PASSWORD)
            .await
            .status
            .is_redirection()
    );
    // Activated (a credential exists) but never signed in.
    assert_eq!(credential_rows(&a.state, bea).await, 1);
    stepped_up(&a.state, &a.session).await;
    let refused = post_issue(&a, bea).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN, "{}", refused.body);
    // The account list page still renders for a non-issuing viewer.
    assert_eq!(get_as(&a, "/admin/users").await.status, StatusCode::OK);
    let _ = get(&a.state, "/admin/login").await;
}

// ── D4 / D6 / D9: the writers of `credentials`, pinned ────────────────

/// Production source text of `text`: comment lines dropped and every
/// `#[cfg(test)]` module removed, so only code that ships is scanned.
fn production_source(text: &str) -> String {
    let bytes: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let starts = |i: usize, pat: &str| -> bool {
        let p: Vec<char> = pat.chars().collect();
        bytes.len() >= i + p.len() && bytes[i..i + p.len()] == p[..]
    };
    while i < bytes.len() {
        if starts(i, "#[cfg(test)]") {
            // Skip to the first `{` (an inline module: match its braces) or
            // `;` (an out-of-line `mod x;`), whichever comes first.
            let mut j = i;
            while j < bytes.len() && bytes[j] != '{' && bytes[j] != ';' {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == '{' {
                let mut depth = 0i32;
                let mut in_str = false;
                while j < bytes.len() {
                    let c = bytes[j];
                    if in_str {
                        if c == '\\' {
                            j += 1;
                        } else if c == '"' {
                            in_str = false;
                        }
                    } else if c == '"' {
                        in_str = true;
                    } else if c == '\'' && j + 2 < bytes.len() && bytes[j + 2] == '\'' {
                        j += 2;
                    } else if c == '{' {
                        depth += 1;
                    } else if c == '}' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    j += 1;
                }
            }
            i = j + 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every non-test `.rs` file under `crates/*/src`, with its production text.
fn production_sources() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).expect("read_dir").flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if p.is_dir() {
                if name != "tests" {
                    walk(&p, out);
                }
            } else if name.ends_with(".rs") && name != "tests.rs" && !name.starts_with("tests_") {
                out.push(p);
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for krate in std::fs::read_dir(&root).expect("crates").flatten() {
        let src = krate.path().join("src");
        if src.is_dir() {
            walk(&src, &mut files);
        }
    }
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(&root)
                .expect("under crates")
                .to_string_lossy()
                .replace('\\', "/");
            let text = std::fs::read_to_string(&p).expect("read");
            (rel, production_source(&text))
        })
        .collect()
}

#[test]
fn r115_s2_credentials_writers_are_the_allowlist() {
    // Four production writers of `credentials`, and the one named exception.
    // A fifth fails here, in CI, rather than in review (RFC 115 D4).
    let pattern = regex_lite_count;
    let mut found: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for (rel, text) in production_sources() {
        let n = pattern(&text);
        if n > 0 {
            found.insert(rel, n);
        }
    }
    let expected: std::collections::BTreeMap<String, usize> = [
        // The setup wizard and headless `sui-id setup`: the operator's own
        // first account, through one raw upsert.
        ("sui-id-core/src/setup.rs", 1),
        // U09 (self-service change) and U10 (completion of a reset link).
        ("sui-id-store/src/commands.rs", 2),
        // The two SQL statements that implement `upsert` and `upsert_within_tx`.
        ("sui-id-store/src/repos/credentials.rs", 2),
        // D6, the named exception: `--dev` seeds fixture users with printed
        // passwords. It runs only under the `--dev` runtime flag.
        ("sui-id/src/runtime/dev_mode.rs", 1),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    assert_eq!(
        found, expected,
        "the set of production writers of `credentials` changed; a new writer needs the \
         architect's decision (RFC 115 D4/D6), not an edit to this list"
    );
}

/// Count matches of the writer pattern without a regex dependency:
/// `credentials::upsert` / `credentials::upsert_within_tx`, and raw SQL that
/// inserts into, updates or deletes from the `credentials` table.
fn regex_lite_count(text: &str) -> usize {
    let mut n = text.matches("credentials::upsert").count();
    for verb in ["INTO", "UPDATE", "FROM"] {
        for kw in ["INSERT", "INSERT OR REPLACE", "UPDATE", "DELETE"] {
            let phrase = match (kw, verb) {
                ("INSERT", "INTO") | ("INSERT OR REPLACE", "INTO") => {
                    format!("{kw} {verb} credentials")
                }
                ("UPDATE", "UPDATE") => "UPDATE credentials".to_owned(),
                ("DELETE", "FROM") => "DELETE FROM credentials".to_owned(),
                _ => continue,
            };
            // Whole-word on the table name: `credentials(` / `credentials ` /
            // `credentials\n`, not `credentials_x` or `user_webauthn_credentials`.
            for (idx, _) in text.match_indices(&phrase) {
                let after = text[idx + phrase.len()..].chars().next();
                if !matches!(after, Some(c) if c.is_alphanumeric() || c == '_') {
                    n += 1;
                }
            }
        }
    }
    n
}

#[test]
fn r115_s2_the_retired_event_and_the_password_field_are_gone_from_production_code() {
    // The grep-proof for D9 (`user.create_warned_hibp`) and D4 (no password
    // at creation), as a test.
    for (rel, text) in production_sources() {
        assert!(
            !text.contains("create_warned_hibp") && !text.contains("CreateWarnedHibp"),
            "{rel} still mentions the retired user.create_warned_hibp"
        );
    }
    let users = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../sui-id-core/src/identity/admin/users.rs"),
    )
    .expect("users.rs");
    let spec = users
        .split("pub struct CreateUserSpec")
        .nth(1)
        .and_then(|rest| rest.split('}').next())
        .expect("CreateUserSpec body");
    assert!(
        !spec.contains("password"),
        "CreateUserSpec must have no password field: {spec}"
    );
}
