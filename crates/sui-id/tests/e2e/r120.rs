//! RFC 120 — consent and the setup wizard must prove who is asking.
//!
//! Each test states an invariant and asserts it through the real router. The
//! ones marked `fails before` are the D7 evidence: they were run against the
//! tree this RFC starts from and fail there.
//!
//! Invariants:
//!
//! - The subject of a consent decision, and the authentication methods of the
//!   code it yields, come from the session that answers and from nothing the
//!   request carries (D1, D2).
//! - The consent state cookie is integrity-protected, bound to a session and
//!   scoped to its path (D3).
//! - The refusal branch validates its redirect target and encodes what it
//!   appends (D4).
//! - A route that changes server-wide security posture requires an
//!   administrator, CSRF and a rate limit, and audits the change (D5).

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};
use sui_id_shared::AuthMethod;
use sui_id_store::models::{ConsentPolicy, HibpMode};
use tower::ServiceExt;

use super::common::*;

const REDIRECT: &str = "https://rp.test/cb";
const BOB_PASSWORD: &str = "bob-very-strong-password";

// ---------- fixtures ----------

struct World {
    state: AppState,
    alice_session: String,
    alice: String,
    bob_session: String,
    bob: String,
    client_id: String,
}

async fn world() -> World {
    let state = test_app();
    let alice_session = complete_setup_and_login(&state).await;
    let alice_id = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("alice")
        .id;
    let (client_id, _secret) = create_client(&state, &alice_session).await;
    // Ask the user every time, so every flow below reaches the consent screen.
    sui_id_store::repos::clients::update_consent_policy(
        &state.db,
        client_id.parse().expect("client id"),
        ConsentPolicy::Always,
        chrono::Utc::now(),
    )
    .await
    .expect("consent policy");

    create_user_with_password(
        &state.db,
        &state.clock,
        &admin_actor_for(alice_id),
        sui_id_core::admin::CreateUserSpec {
            username: "bob",
            display_name: None,
            email: None,
            is_admin: false,
        },
        BOB_PASSWORD,
    )
    .await
    .expect("create bob");
    let bob_id = sui_id_store::repos::users::find_by_username(&state.db, "bob")
        .await
        .expect("bob")
        .id;
    let bob_session = sui_id_core::session::login(
        &state.db,
        &state.clock,
        "bob",
        BOB_PASSWORD,
        state.config.security.max_lockout.as_secs(),
    )
    .await
    .expect("bob signs in")
    .id
    .to_string();

    World {
        state,
        alice_session,
        alice: alice_id.to_string(),
        bob_session,
        bob: bob_id.to_string(),
        client_id,
    }
}

fn authorize_uri(client_id: &str, rp_state: &str) -> String {
    let (_, challenge) = pkce_pair();
    format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri={}&response_type=code\
         &scope=openid&state={}&code_challenge={challenge}&code_challenge_method=S256",
        urlencode(REDIRECT),
        urlencode(rp_state),
    )
}

/// What the consent screen hands the browser.
struct Screen {
    consent_cookie: String,
    csrf: String,
    set_cookies: Vec<String>,
}

async fn consent_screen(
    state: &AppState,
    client_id: &str,
    session: &str,
    rp_state: &str,
) -> Screen {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(authorize_uri(client_id, rp_state))
                .header(header::COOKIE, format!("sui_id_session={session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("authorize");
    assert_eq!(resp.status(), StatusCode::OK, "the consent screen");
    let set_cookies = resp
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().expect("utf8").to_owned())
        .collect();
    Screen {
        consent_cookie: extract_set_cookie(resp.headers(), "sui_id_consent")
            .expect("the screen sets its state cookie"),
        csrf: extract_csrf_cookie(resp.headers()).expect("the screen sets a csrf cookie"),
        set_cookies,
    }
}

/// `POST /oauth2/consent`. Cookies are whatever the caller chooses to present.
async fn post_consent(
    state: &AppState,
    session: Option<&str>,
    consent_cookie: Option<&str>,
    csrf: &str,
    decision: &str,
) -> axum::response::Response {
    let mut cookies = Vec::new();
    if let Some(s) = session {
        cookies.push(format!("sui_id_session={s}"));
    }
    if let Some(c) = consent_cookie {
        cookies.push(format!("sui_id_consent={c}"));
    }
    cookies.push(format!("sui_id_csrf={csrf}"));
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/consent")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::COOKIE, cookies.join("; "))
                .body(Body::from(format!("decision={decision}&_csrf={csrf}")))
                .expect("req"),
        )
        .await
        .expect("consent post")
}

fn location(resp: &axum::response::Response) -> String {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned()
}

/// A state cookie value that this server did not issue.
fn unissued_state(client_id: &str, user: &str) -> String {
    let (_, challenge) = pkce_pair();
    format!(
        r#"{{"user_id":"{user}","client_id":"{client_id}","redirect_uri":"{REDIRECT}","scope":"openid","state":"s","nonce":null,"code_challenge":"{challenge}","code_challenge_method":"S256","auth_methods":["pwd","webauthn"]}}"#
    )
}

/// `(user_id, auth_methods)` of every authorization code issued so far.
async fn issued_codes(state: &AppState) -> Vec<(String, String)> {
    state
        .db
        .with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT user_id, auth_methods FROM auth_codes")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .expect("read auth_codes")
}

async fn consent_grants(state: &AppState) -> usize {
    state
        .db
        .with_conn(|conn| {
            Ok(
                conn.query_row("SELECT COUNT(*) FROM user_consent", [], |r| {
                    r.get::<_, i64>(0)
                })? as usize,
            )
        })
        .await
        .expect("count user_consent")
}

fn is_code_redirect(resp: &axum::response::Response) -> bool {
    resp.status().is_redirection() && location(resp).contains("code=")
}

// ---------- consent: the legitimate flow still works ----------

#[tokio::test]
async fn consent_approval_issues_a_code_for_the_sessions_user_with_the_sessions_methods() {
    let w = world().await;
    // Give alice's session two factors, so a code that recorded anything other
    // than the session's own record would differ.
    let methods = serde_json::to_string(&[AuthMethod::Pwd, AuthMethod::Totp]).unwrap();
    let sid = w.alice_session.clone();
    w.state
        .db
        .with_conn(move |conn| {
            conn.execute(
                "UPDATE sessions SET auth_methods = ?1 WHERE id = ?2",
                rusqlite::params![methods, sid],
            )?;
            Ok(())
        })
        .await
        .unwrap();

    let screen = consent_screen(&w.state, &w.client_id, &w.alice_session, "rp-state").await;
    let resp = post_consent(
        &w.state,
        Some(&w.alice_session),
        Some(&screen.consent_cookie),
        &screen.csrf,
        "approve",
    )
    .await;
    assert!(
        is_code_redirect(&resp),
        "got {} {}",
        resp.status(),
        location(&resp)
    );
    assert!(location(&resp).starts_with(REDIRECT), "{}", location(&resp));
    assert!(
        location(&resp).contains("state=rp%2Dstate"),
        "{}",
        location(&resp)
    );

    let codes = issued_codes(&w.state).await;
    assert_eq!(codes.len(), 1);
    assert_eq!(codes[0].0, w.alice);
    assert_eq!(codes[0].1, r#"["pwd","totp"]"#);
    assert_eq!(consent_grants(&w.state).await, 1);
}

#[tokio::test]
async fn consent_denial_redirects_to_the_registered_target_with_access_denied() {
    let w = world().await;
    let screen = consent_screen(&w.state, &w.client_id, &w.alice_session, "rp-state").await;
    let resp = post_consent(
        &w.state,
        Some(&w.alice_session),
        Some(&screen.consent_cookie),
        &screen.csrf,
        "deny",
    )
    .await;
    assert!(resp.status().is_redirection());
    let loc = location(&resp);
    assert!(
        loc.starts_with(&format!("{REDIRECT}?error=access_denied")),
        "{loc}"
    );
    assert!(issued_codes(&w.state).await.is_empty());
    assert_eq!(consent_grants(&w.state).await, 0);
}

// ---------- consent: D1, D2, D3 (fails before) ----------

#[tokio::test]
async fn consent_requires_a_session() {
    // fails before
    let w = world().await;
    let screen = consent_screen(&w.state, &w.client_id, &w.alice_session, "rp-state").await;
    let resp = post_consent(
        &w.state,
        None,
        Some(&screen.consent_cookie),
        &screen.csrf,
        "approve",
    )
    .await;
    assert!(
        !is_code_redirect(&resp),
        "an answer with no session must not yield a code: {} {}",
        resp.status(),
        location(&resp)
    );
    assert!(issued_codes(&w.state).await.is_empty());
    assert_eq!(consent_grants(&w.state).await, 0);
}

#[tokio::test]
async fn consent_answer_requires_the_csrf_pair() {
    let w = world().await;
    let screen = consent_screen(&w.state, &w.client_id, &w.alice_session, "rp-state").await;
    // The form field does not match the cookie.
    let resp = build_router(w.state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/oauth2/consent")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!(
                        "sui_id_session={}; sui_id_consent={}; sui_id_csrf={}",
                        w.alice_session, screen.consent_cookie, screen.csrf
                    ),
                )
                .body(Body::from("decision=approve&_csrf=not-the-token"))
                .expect("req"),
        )
        .await
        .expect("consent post");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert!(issued_codes(&w.state).await.is_empty());
    assert_eq!(consent_grants(&w.state).await, 0);
}

#[tokio::test]
async fn consent_state_is_bound_to_the_session_that_received_it() {
    // fails before
    let w = world().await;
    let screen = consent_screen(&w.state, &w.client_id, &w.alice_session, "rp-state").await;
    // Bob has a live session of his own; the value was not issued to it.
    let resp = post_consent(
        &w.state,
        Some(&w.bob_session),
        Some(&screen.consent_cookie),
        &screen.csrf,
        "approve",
    )
    .await;
    assert!(
        !is_code_redirect(&resp),
        "{} {}",
        resp.status(),
        location(&resp)
    );
    assert!(issued_codes(&w.state).await.is_empty());
    assert_eq!(consent_grants(&w.state).await, 0);
}

#[tokio::test]
async fn consent_state_the_server_did_not_issue_is_refused() {
    // fails before
    let w = world().await;
    for session in [&w.alice_session, &w.bob_session] {
        for subject in [&w.alice, &w.bob] {
            let resp = post_consent(
                &w.state,
                Some(session),
                Some(&unissued_state(&w.client_id, subject)),
                "csrf-value",
                "approve",
            )
            .await;
            assert!(
                !is_code_redirect(&resp),
                "{} {}",
                resp.status(),
                location(&resp)
            );
        }
    }
    assert!(issued_codes(&w.state).await.is_empty());
    assert_eq!(consent_grants(&w.state).await, 0);
}

#[tokio::test]
async fn consent_state_cookie_is_scoped_to_its_path_and_not_readable_by_script() {
    // fails before
    let w = world().await;
    let screen = consent_screen(&w.state, &w.client_id, &w.alice_session, "rp-state").await;
    let cookie = screen
        .set_cookies
        .iter()
        .find(|c| c.starts_with("sui_id_consent="))
        .expect("state cookie");
    assert!(cookie.contains("Path=/oauth2/consent"), "{cookie}");
    assert!(cookie.contains("HttpOnly"), "{cookie}");
    assert!(cookie.contains("SameSite=Lax"), "{cookie}");
    assert!(cookie.contains("Max-Age=300"), "{cookie}");
    // No identity travels in it.
    assert!(!cookie.contains(&w.alice), "{cookie}");
}

// ---------- consent: D4 (fails before) ----------

#[tokio::test]
async fn consent_denial_re_validates_its_target_against_the_client() {
    // fails before
    let w = world().await;
    let screen = consent_screen(&w.state, &w.client_id, &w.alice_session, "rp-state").await;
    // The client stops being usable between the screen and the answer.
    sui_id_store::repos::clients::set_disabled(&w.state.db, w.client_id.parse().unwrap(), true)
        .await
        .unwrap();
    let resp = post_consent(
        &w.state,
        Some(&w.alice_session),
        Some(&screen.consent_cookie),
        &screen.csrf,
        "deny",
    )
    .await;
    assert!(
        !resp.status().is_redirection(),
        "a target that no longer validates must not be redirected to: {} {}",
        resp.status(),
        location(&resp)
    );
}

#[tokio::test]
async fn consent_denial_encodes_what_it_appends() {
    // fails before
    let w = world().await;
    let tricky = "a&b=c d#e";
    let screen = consent_screen(&w.state, &w.client_id, &w.alice_session, tricky).await;
    let resp = post_consent(
        &w.state,
        Some(&w.alice_session),
        Some(&screen.consent_cookie),
        &screen.csrf,
        "deny",
    )
    .await;
    let loc = location(&resp);
    assert!(resp.status().is_redirection(), "{}", resp.status());
    assert!(
        loc.ends_with(&format!("&state={}", urlencode(tricky))),
        "{loc}"
    );
    assert!(!loc.contains("a&b=c"), "{loc}");
}

// ---------- setup wizard steps: D5 ----------

async fn settings(state: &AppState) -> sui_id_store::models::ServerSettingsRow {
    sui_id_store::repos::server_settings::get(&state.db)
        .await
        .expect("settings")
}

async fn settings_audit(state: &AppState) -> Vec<(String, Option<String>, Option<String>)> {
    state
        .db
        .with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT action, actor, note FROM audit_log WHERE action LIKE 'settings.%' ORDER BY seq",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, Option<String>>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows)
        })
        .await
        .expect("read audit")
}

const STEPS: [(&str, &str, &str); 2] = [
    ("/setup/hibp", "hibp_mode", "off"),
    ("/setup/lang", "lang", "en"),
];

async fn post_step(
    state: &AppState,
    path: &str,
    field: &str,
    value: &str,
    session: Option<&str>,
    csrf: Option<&str>,
) -> axum::response::Response {
    let mut cookies = Vec::new();
    if let Some(s) = session {
        cookies.push(format!("sui_id_session={s}"));
    }
    let mut body = format!("{field}={value}");
    if let Some(c) = csrf {
        cookies.push(format!("sui_id_csrf={c}"));
        body.push_str(&format!("&_csrf={c}"));
    }
    let mut req = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if !cookies.is_empty() {
        req = req.header(header::COOKIE, cookies.join("; "));
    }
    build_router(state.clone())
        .oneshot(req.body(Body::from(body)).expect("req"))
        .await
        .expect("step post")
}

fn unchanged(
    before: &sui_id_store::models::ServerSettingsRow,
    after: &sui_id_store::models::ServerSettingsRow,
) -> bool {
    before.default_lang == after.default_lang && before.hibp_mode == after.hibp_mode
}

#[tokio::test]
async fn setup_steps_refuse_a_caller_with_no_session_once_initialized() {
    // fails before
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;
    let before = settings(&state).await;
    for (path, field, value) in STEPS {
        let resp = post_step(&state, path, field, value, None, None).await;
        assert!(
            resp.status().is_redirection() && location(&resp) == "/admin/login",
            "{path}: {} {}",
            resp.status(),
            location(&resp)
        );
        // With a CSRF pair too: the pair proves nothing about who is asking.
        let resp = post_step(&state, path, field, value, None, Some("t")).await;
        assert!(
            resp.status().is_redirection() && location(&resp) == "/admin/login",
            "{path}: {} {}",
            resp.status(),
            location(&resp)
        );
    }
    assert!(unchanged(&before, &settings(&state).await));
    assert!(settings_audit(&state).await.is_empty());
}

#[tokio::test]
async fn setup_steps_refuse_a_user_who_is_not_an_administrator() {
    // fails before
    let w = world().await;
    let before = settings(&w.state).await;
    for (path, field, value) in STEPS {
        let resp = post_step(
            &w.state,
            path,
            field,
            value,
            Some(&w.bob_session),
            Some("t"),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{path}");
    }
    assert!(unchanged(&before, &settings(&w.state).await));
    assert!(settings_audit(&w.state).await.is_empty());
}

#[tokio::test]
async fn setup_steps_require_csrf_from_an_administrator() {
    // fails before
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let before = settings(&state).await;
    for (path, field, value) in STEPS {
        let resp = post_step(&state, path, field, value, Some(&session), None).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{path}");
    }
    assert!(unchanged(&before, &settings(&state).await));
    assert!(settings_audit(&state).await.is_empty());
}

#[tokio::test]
async fn setup_steps_by_an_administrator_apply_and_audit_the_old_and_new_value() {
    // fails before (no audit row)
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .unwrap()
        .id
        .to_string();
    let before = settings(&state).await;
    assert_eq!(before.hibp_mode, HibpMode::Warn);
    assert_eq!(before.default_lang, "ja");
    let csrf = fetch_csrf(&state, &session).await;

    let resp = post_step(
        &state,
        "/setup/hibp",
        "hibp_mode",
        "off",
        Some(&session),
        Some(&csrf),
    )
    .await;
    assert!(resp.status().is_redirection(), "{}", resp.status());
    assert_eq!(location(&resp), "/setup/done");
    let resp = post_step(
        &state,
        "/setup/lang",
        "lang",
        "en",
        Some(&session),
        Some(&csrf),
    )
    .await;
    assert!(resp.status().is_redirection(), "{}", resp.status());
    assert_eq!(location(&resp), "/setup/hibp");

    let after = settings(&state).await;
    assert_eq!(after.hibp_mode, HibpMode::Off);
    assert_eq!(after.default_lang, "en");

    let audit = settings_audit(&state).await;
    let hibp = audit
        .iter()
        .find(|(a, _, _)| a == "settings.hibp_mode.changed")
        .expect("hibp change is audited");
    assert_eq!(hibp.1.as_deref(), Some(admin.as_str()));
    assert_eq!(hibp.2.as_deref(), Some("old=warn new=off"));
    let lang = audit
        .iter()
        .find(|(a, _, _)| a == "settings.default_language.changed")
        .expect("language change is audited");
    assert_eq!(lang.1.as_deref(), Some(admin.as_str()));
    assert_eq!(lang.2.as_deref(), Some("old=ja new=en"));
}

#[tokio::test]
async fn setup_steps_are_rate_limited() {
    // fails before
    for (path, field, value) in STEPS {
        // A fresh instance per step, so one step's budget is not spent by the other.
        let state = test_app();
        let session = complete_setup_and_login(&state).await;
        let csrf = fetch_csrf(&state, &session).await;
        let mut limited = false;
        for _ in 0..40 {
            let resp = post_step(&state, path, field, value, Some(&session), Some(&csrf)).await;
            if resp.status() == StatusCode::TOO_MANY_REQUESTS {
                limited = true;
                break;
            }
        }
        assert!(limited, "{path}: forty answers in a row were all accepted");
    }
}

#[tokio::test]
async fn setup_steps_do_nothing_before_initialization() {
    let state = test_app();
    let before = settings(&state).await;
    for (path, field, value) in STEPS {
        let resp = post_step(&state, path, field, value, None, None).await;
        assert!(
            !location(&resp).starts_with("/setup/"),
            "{path}: {} {}",
            resp.status(),
            location(&resp)
        );
    }
    assert!(unchanged(&before, &settings(&state).await));
    assert!(settings_audit(&state).await.is_empty());
}

#[tokio::test]
async fn setup_step_forms_carry_the_csrf_pair() {
    // fails before
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    for path in ["/setup/lang", "/setup/hibp"] {
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
            .expect("step form");
        assert_eq!(resp.status(), StatusCode::OK, "{path}");
        let cookie = extract_csrf_cookie(resp.headers()).expect("csrf cookie");
        let html = String::from_utf8_lossy(&read_body(resp.into_body()).await).to_string();
        assert_eq!(extract_csrf_token(&html), cookie, "{path}");
    }
}

#[tokio::test]
async fn setup_step_forms_are_not_shown_to_a_caller_with_no_session() {
    // fails before
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;
    for path in ["/setup/lang", "/setup/hibp"] {
        let resp = build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(path)
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("step form");
        assert!(
            resp.status().is_redirection() && location(&resp) == "/admin/login",
            "{path}: {} {}",
            resp.status(),
            location(&resp)
        );
    }
}
