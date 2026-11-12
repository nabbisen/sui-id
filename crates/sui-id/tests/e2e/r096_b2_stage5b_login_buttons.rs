//! RFC 096-B2 stage 5b — "Sign in with …" on the login page.
//!
//! Until this stage, federated sign-in could only be started by typing
//! `/auth/federated/{slug}/start` into the browser. This file drives the
//! button the login page now renders, through the real HTTP router,
//! against the real mock upstream (`federation_fail_closed`'s harness).

use super::common::*;
use super::federation_fail_closed::{
    TEST_CLIENT_SECRET, add_federation_provider_config, federated_signin_with_next, mock_upstream,
};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::FederationProviderId;
use sui_id_store::models::{FederationProviderRow, ProvisionMode, ValidationStatus};
use sui_id_store::repos::federation_provider;
use tower::ServiceExt;

fn disabled_row(issuer: &str, slug: &str, display_name: &str) -> FederationProviderRow {
    let now = chrono::Utc::now();
    FederationProviderRow {
        id: FederationProviderId::new(),
        slug: slug.into(),
        display_name: display_name.into(),
        issuer: issuer.into(),
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

async fn seed_enabled(
    state: &AppState,
    issuer: &str,
    slug: &str,
    display_name: &str,
) -> FederationProviderId {
    let row = disabled_row(issuer, slug, display_name);
    let provider_id = row.id;
    federation_provider::create_enabled_for_test(&state.db, &row, None)
        .await
        .expect("seed enabled provider");
    provider_id
}

async fn seed_disabled(state: &AppState, issuer: &str, slug: &str, display_name: &str) {
    let row = disabled_row(issuer, slug, display_name);
    federation_provider::create(&state.db, &row, Some(TEST_CLIENT_SECRET))
        .await
        .expect("seed disabled provider");
}

/// `RequestLocale`'s own tier order (`handlers.rs`) is signed-in user
/// preference, then a locale cookie, then `Accept-Language` -- an
/// unauthenticated GET with no cookie falls to the hardcoded `Locale::Ja`
/// default otherwise, so every assertion below on English text sends
/// this header.
async fn get_login(state: &AppState, uri: &str) -> String {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(uri)
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    assert_eq!(resp.status(), StatusCode::OK);
    String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned()
}

/// The `href` of the anchor whose text contains `label_fragment` --
/// a small, deliberately literal scan (not a full HTML parser), matched
/// against this page's own fixed `<a href="…">…</a>` shape.
fn find_href(html: &str, label_fragment: &str) -> Option<String> {
    for (start, _) in html.match_indices("<a href=\"") {
        let rest = &html[start..];
        let end = rest.find("</a>")?;
        let anchor = &rest[..end + "</a>".len()];
        if anchor.contains(label_fragment) {
            let href_start = anchor.find("href=\"")? + "href=\"".len();
            let href_rest = &anchor[href_start..];
            let href_end = href_rest.find('"')?;
            return Some(href_rest[..href_end].to_owned());
        }
    }
    None
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_enabled_provider_appears_and_a_disabled_one_does_not() {
    let state = test_app();
    seed_enabled(&state, "https://enabled.example", "up", "Up").await;
    seed_disabled(&state, "https://disabled.example", "down", "Down").await;

    let body = get_login(&state, "/admin/login").await;
    assert!(body.contains("Sign in with Up"), "{body}");
    assert!(
        body.contains("/auth/federated/up/start"),
        "no link to the enabled provider: {body}"
    );
    assert!(
        !body.contains("Sign in with Down"),
        "a disabled provider must not appear: {body}"
    );
    assert!(
        !body.contains("/auth/federated/down/start"),
        "a disabled provider's link must not appear: {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn providers_are_ordered_by_display_name_not_slug_or_insertion_order() {
    let state = test_app();
    // Inserted in reverse display-name order, with slugs that would sort
    // the other way if the page used `list_enabled`'s own SQL ordering.
    seed_enabled(&state, "https://z.example", "aaa-slug", "Zebra").await;
    seed_enabled(&state, "https://a.example", "zzz-slug", "Apple").await;

    let body = get_login(&state, "/admin/login").await;
    let apple_at = body.find("Apple").expect("Apple present");
    let zebra_at = body.find("Zebra").expect("Zebra present");
    assert!(
        apple_at < zebra_at,
        "Apple must render before Zebra (display_name order): {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn next_with_reserved_characters_survives_the_button_href() {
    let state = test_app();
    seed_enabled(&state, "https://next-test.example", "up", "Up").await;
    let next = "/me/security?x=1&y=2";

    let body = get_login(
        &state,
        &format!(
            "/admin/login?next={}",
            percent_encoding::utf8_percent_encode(next, percent_encoding::NON_ALPHANUMERIC)
        ),
    )
    .await;
    let href = find_href(&body, "Sign in with Up").expect("button href present");
    let (path, query) = href.split_once('?').expect("href carries a next= query");
    assert_eq!(path, "/auth/federated/up/start");
    let decoded = url::form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == "next")
        .map(|(_, v)| v.into_owned())
        .expect("next in the query");
    assert_eq!(decoded, next, "next must decode back exactly: {href}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_display_name_with_markup_characters_renders_as_text_not_markup() {
    let state = test_app();
    seed_enabled(
        &state,
        "https://xss-test.example",
        "up",
        "<script>&\"evil\"</script>",
    )
    .await;

    let body = get_login(&state, "/admin/login").await;
    assert!(
        !body.contains("<script>&\"evil\"</script>"),
        "the raw display_name must not appear unescaped: {body}"
    );
    assert!(
        !body.to_lowercase().contains("<script>alert"),
        "no literal script tag reached the page: {body}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_enabled_providers_renders_no_group_and_no_divider() {
    let state = test_app();
    seed_disabled(&state, "https://none-enabled.example", "down", "Down").await;

    let body = get_login(&state, "/admin/login").await;
    assert!(
        !body.contains("divider-with-label"),
        "no divider when nothing is offered: {body}"
    );
    assert!(!body.contains("Sign in with"), "{body}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_password_failure_still_lists_the_provider() {
    let state = test_app();
    complete_setup_and_login(&state).await;
    seed_enabled(&state, "https://password-fail.example", "up", "Up").await;

    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(header::ACCEPT_LANGUAGE, "en")
                .body(Body::from(format!(
                    "username={}&password=definitely-wrong",
                    urlencode(USERNAME)
                )))
                .expect("req"),
        )
        .await
        .expect("login POST");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = String::from_utf8_lossy(&read_body(resp.into_body()).await).into_owned();
    assert!(
        body.contains("Sign in with Up"),
        "a failed password attempt must still list the provider: {body}"
    );
}

// ── End to end: the reason this stage exists ─────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sign_in_started_from_the_rendered_button_completes_then_disappears_once_disabled() {
    let mut state = test_app();
    let admin_session = complete_setup_and_login(&state).await;
    state.http_client = std::sync::Arc::new(Some(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    )));
    let (issuer, nonce_slot) = mock_upstream().await;
    add_federation_provider_config(&mut state, &issuer);
    let user = super::federation_fail_closed::linked_user(&state, &issuer).await;
    let provider_id = federation_provider::get_by_slug(&state.db, "up")
        .await
        .expect("seeded by linked_user")
        .id;

    let body = get_login(&state, "/admin/login").await;
    let href = find_href(&body, "Sign in with Up").expect("button present in the rendered HTML");
    assert_eq!(
        href, "/auth/federated/up/start",
        "no next was supplied, so the href carries none"
    );

    // Follow it: GET the href taken from the page, then complete the
    // callback exactly as `federated_signin` already proves works for
    // this fixture -- the point here is that the *page* produced the
    // right address, not a re-test of the protocol stage 4 already
    // covers.
    let start_resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&href)
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("start");
    assert!(start_resp.status().is_redirection());

    let o = federated_signin_with_next(&state, &nonce_slot, "").await;
    assert_eq!(o.location, "/admin");
    assert!(o.session_cookie.is_some());
    assert_eq!(sessions_of(&state, user).await, 1);

    // Disable it through S5's own admin surface.
    let list_resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/federation-providers")
                .header(header::COOKIE, format!("sui_id_session={admin_session}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("list GET");
    let csrf = extract_set_cookie(list_resp.headers(), "sui_id_csrf").expect("csrf");
    let disable_resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/admin/federation-providers/{provider_id}/disable"))
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={admin_session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(format!("_csrf={csrf}&_confirmed=1")))
                .expect("req"),
        )
        .await
        .expect("disable POST");
    assert!(disable_resp.status().is_redirection());

    let body_after = get_login(&state, "/admin/login").await;
    assert!(
        !body_after.contains("Sign in with Up"),
        "the button must be gone once the provider is disabled: {body_after}"
    );
}

async fn sessions_of(state: &AppState, user: sui_id_shared::ids::UserId) -> i64 {
    state
        .db
        .with_conn(move |c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM sessions WHERE user_id = ?1",
                [user.to_string()],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("count sessions")
}

// ── Through an OIDC authorization ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn starting_from_oauth2_authorize_lands_on_the_login_page_with_the_button() {
    let mut state = test_app();
    let session = complete_setup_and_login(&state).await;
    let (client_id, _secret) = create_client(&state, &session).await;
    state.http_client = std::sync::Arc::new(Some(super::tls_mock::federation_test_client(
        sui_id::resolver::ValidatingResolver,
    )));
    let (issuer, nonce_slot) = mock_upstream().await;
    add_federation_provider_config(&mut state, &issuer);
    super::federation_fail_closed::linked_user(&state, &issuer).await;

    let (_verifier, challenge) = pkce_pair();
    let auth_url = format!(
        "/oauth2/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Frp.test%2Fcb\
         &response_type=code&scope=openid&state=xyz&nonce=n0&code_challenge={challenge}\
         &code_challenge_method=S256"
    );
    // Deliberately unauthenticated: no session cookie.
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(&auth_url)
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("authorize");
    assert!(resp.status().is_redirection(), "status {}", resp.status());
    let login_location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("redirected to login")
        .to_owned();
    assert!(login_location.starts_with("/admin/login?next="));

    let body = get_login(&state, &login_location).await;
    let href = find_href(&body, "Sign in with Up").expect("button present on the login page");
    // The button's own `next` is this exact `/oauth2/authorize?...` URL,
    // percent-decoded back to it -- the same round trip
    // `next_with_reserved_characters_survives_the_button_href` already
    // pins against a synthetic value, now against the real one
    // `/oauth2/authorize` itself redirected with.
    let (path, query) = href.split_once('?').expect("href carries next=");
    assert_eq!(path, "/auth/federated/up/start");
    let decoded_next = url::form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == "next")
        .map(|(_, v)| v.into_owned())
        .expect("next in the query");
    let expected_next = login_location
        .split_once("next=")
        .map(|(_, v)| {
            percent_encoding::percent_decode_str(v)
                .decode_utf8()
                .expect("utf8")
                .into_owned()
        })
        .expect("next in the login redirect");
    assert_eq!(decoded_next, expected_next);

    // Following the button all the way through: sign in, and see where
    // it lands. `federated_signin_with_next` starts `/auth/federated/
    // up/start?next=<decoded_next>` itself (the same URL the href carries)
    // and completes the callback.
    let o = federated_signin_with_next(&state, &nonce_slot, &decoded_next).await;
    assert!(o.session_cookie.is_some(), "sign-in itself must succeed");
    // S5b review: the sign-in now resumes the authorization it started
    // from, as the password path does. Before, both direct-success
    // branches landed on `/admin` and dropped the attempt's `next_path`.
    assert_eq!(
        o.location, decoded_next,
        "a federated sign-in started from an OIDC authorization must return to it"
    );
}
