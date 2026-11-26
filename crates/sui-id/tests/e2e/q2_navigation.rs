//! RFC 149 Q2 (`0.95.0-q2-the-chrome`), §6: the grouped sidebar closes RFC
//! 149's first condition ("every admin page belongs to exactly one
//! navigation group") and RFC 151's last one (the password minimum is
//! found from *Sign-in settings*). Tests 1, 2, 3, 4, 6 and 7 of that
//! dispatch's §6 table live here; test 5 (the account menu) and test 8 (no
//! `title=`/`role="menu"`) do too. Test 9 (no `kv-list`) is a source scan,
//! beside `pages::repair_checks` in `sui-id-web` instead (G21).
//!
//! The sidebar renders twice (RFC 149 §2: one copy per breakpoint, each
//! `display: none` at the other width, so exactly one is ever in the
//! accessibility tree). Both copies carry identical content, so every
//! assertion here that reads `aria-current="page"` collects the *set* of
//! hrefs so marked rather than counting occurrences: one href, found
//! (typically) twice, is "exactly one item is current".

use super::common::*;
use super::r120_routes::routes;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::{AppState, build_router};
use sui_id_shared::ids::UserId;
use sui_id_store::models::{FederationProviderRow, ProvisionMode, ValidationStatus};
use sui_id_store::repos::federation_provider;
use tower::ServiceExt;

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

async fn post_form(
    state: &AppState,
    uri: &str,
    session: &str,
    csrf: &str,
    body: &str,
) -> axum::response::Response {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(uri)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .header(
                    header::COOKIE,
                    format!("sui_id_session={session}; sui_id_csrf={csrf}"),
                )
                .body(Body::from(body.to_owned()))
                .expect("req"),
        )
        .await
        .expect("POST")
}

/// Signs a plain, non-admin user in through the same `/admin/login` form
/// the console uses (same shape as `n2c_account_chrome.rs`'s own
/// `login_as`: `login_post` establishes a session for any role once the
/// destination is under `/me/`).
async fn login_as(state: &AppState, username: &str, password: &str) -> String {
    let resp = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username={username}&password={password}&next=/me/security/overview"
                )))
                .expect("req"),
        )
        .await
        .expect("login POST");
    assert!(
        resp.status().is_redirection(),
        "expected the non-admin login to redirect, got {}",
        resp.status()
    );
    axum::http::HeaderMap::get_all(resp.headers(), header::SET_COOKIE)
        .iter()
        .find_map(|v| {
            v.to_str()
                .ok()?
                .strip_prefix("sui_id_session=")?
                .split(';')
                .next()
                .map(str::to_owned)
        })
        .expect("session cookie set")
}

fn location(resp: &axum::response::Response) -> String {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned()
}

async fn html_of(resp: axum::response::Response) -> String {
    let body = read_body(resp.into_body()).await;
    String::from_utf8_lossy(&body).to_string()
}

/// `html` from `<body` onward: every page embeds its full stylesheet
/// inline in `<head>`, including this very file's own doc comments
/// (which, for example, say `<h1>` and `role="menu"` while explaining
/// why the chrome no longer has either) — searching the whole page for
/// those strings would match the comment, not the markup.
fn body_only(html: &str) -> &str {
    html.find("<body").map(|i| &html[i..]).unwrap_or(html)
}

/// Every `href="..."` on an `<a class="app-sidebar__link" ...>` tag that
/// also carries `aria-current="page"`, one entry per occurrence (the
/// sidebar's two responsive copies each contribute one when an item is
/// current).
fn sidebar_current_hrefs(html: &str) -> Vec<String> {
    let marker = r#"class="app-sidebar__link""#;
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = html[from..].find(marker) {
        let tag_start = html[..from + rel].rfind('<').unwrap_or(from + rel);
        let tag_end = html[tag_start..]
            .find('>')
            .map(|e| tag_start + e)
            .unwrap_or(html.len());
        let tag = &html[tag_start..tag_end];
        from = tag_end + 1;
        if !tag.contains(r#"aria-current="page""#) {
            continue;
        }
        let Some(hs) = tag.find("href=\"") else {
            continue;
        };
        let hs = hs + "href=\"".len();
        let Some(he) = tag[hs..].find('"') else {
            continue;
        };
        out.push(tag[hs..hs + he].to_owned());
    }
    out
}

/// Asserts that exactly one distinct href is marked current in `html`,
/// and that it is `expected_href`.
fn assert_exactly_current(html: &str, expected_href: &str, route: &str) {
    let hrefs = sidebar_current_hrefs(html);
    assert!(
        !hrefs.is_empty(),
        "{route}: no sidebar link has aria-current=\"page\""
    );
    let unique: std::collections::BTreeSet<&str> = hrefs.iter().map(String::as_str).collect();
    assert_eq!(
        unique.len(),
        1,
        "{route}: more than one distinct item marked current: {unique:?}"
    );
    assert_eq!(
        *unique.iter().next().unwrap(),
        expected_href,
        "{route}: wrong item marked current"
    );
}

fn disabled_provider_row(slug: &str) -> FederationProviderRow {
    let now = chrono::Utc::now();
    FederationProviderRow {
        id: sui_id_shared::ids::FederationProviderId::new(),
        slug: slug.into(),
        display_name: "Q2 Up".into(),
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

/// Every fixture every test below needs, built once. Mirrors
/// `n2b_account_menu.rs`'s own seeding (that file cannot be reused
/// directly: its helpers are module-private), with one addition — a
/// second signed-out check needs no fixture, and is done inline.
struct Seed {
    session: String,
    client_id: String,
    target_user: UserId,
    /// A session for `target_user` (an ordinary, non-admin account) —
    /// `/me/security/overview` renders with `Chrome::Account` only for a
    /// non-admin; an admin's own `/me` pages render with the console's
    /// chrome instead (§1: "shown with the console's chrome to an
    /// administrator").
    non_admin_session: String,
    signing_key_id: sui_id_shared::ids::SigningKeyId,
    disabled_provider_id: sui_id_shared::ids::FederationProviderId,
    enabled_provider_id: sui_id_shared::ids::FederationProviderId,
    linked_user: UserId,
    email_confirm_path: String,
    /// The CSRF token the pending email change was created under — the
    /// confirm step must reuse it, not a freshly fetched one.
    email_csrf: String,
}

async fn seed(state: &AppState) -> Seed {
    let session = complete_setup_and_login(state).await;
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin user");

    let (client_id, _secret) = create_client(state, &session).await;

    let target = create_user_with_password(
        &state.db,
        &sui_id_core::time::system_clock(),
        &admin_actor_for(admin.id),
        sui_id_core::admin::CreateUserSpec {
            username: "q2-target",
            display_name: None,
            email: None,
            is_admin: false,
        },
        "q2-target-password-1",
    )
    .await
    .expect("create target user");
    let non_admin_session = login_as(state, "q2-target", "q2-target-password-1").await;

    let signing_key_id = sui_id_core::admin::list_signing_keys(&state.db)
        .await
        .expect("list signing keys")
        .into_iter()
        .next()
        .expect("setup seeds at least one signing key")
        .id;

    let disabled_row = disabled_provider_row("q2-enable");
    let disabled_provider_id = disabled_row.id;
    federation_provider::create(&state.db, &disabled_row, None)
        .await
        .expect("seed disabled provider");

    let mut enabled_row = disabled_provider_row("q2-disable");
    enabled_row.enabled = true;
    let enabled_provider_id = enabled_row.id;
    federation_provider::create_enabled_for_test(&state.db, &enabled_row, None)
        .await
        .expect("seed enabled provider");

    let linked_user = create_user_with_password(
        &state.db,
        &sui_id_core::time::system_clock(),
        &admin_actor_for(admin.id),
        sui_id_core::admin::CreateUserSpec {
            username: "q2-linked",
            display_name: None,
            email: None,
            is_admin: false,
        },
        "q2-linked-password-1",
    )
    .await
    .expect("create linked user");
    state
        .db
        .with_conn({
            let provider_id = enabled_provider_id;
            let user_id = linked_user.id;
            move |c| {
                Ok(c.execute(
                    "INSERT INTO federation_link \
                     (user_id, provider_id, upstream_sub, linked_at, last_seen_at) \
                     VALUES (?1, ?2, 'q2-sub', ?3, ?3)",
                    rusqlite::params![
                        user_id.to_string(),
                        provider_id.to_string(),
                        chrono::Utc::now().to_rfc3339(),
                    ],
                )?)
            }
        })
        .await
        .expect("link user to provider");

    // RFC 090 P5: the confirm step rebinds the pending change to the
    // CSRF token active when it was created, so `email_csrf` (not a
    // freshly re-fetched one) must be reused by whoever confirms it.
    let email_csrf = fetch_csrf(state, &session).await;
    let email_resp = post_form(
        state,
        "/admin/settings/email",
        &session,
        &email_csrf,
        &format!(
            "_csrf={email_csrf}&host=smtp.example.com&port=587&tls_mode=starttls\
             &username=&password={}&from_address=noreply%40example.com\
             &from_name=&base_url=https%3A%2F%2Fexample.com",
            urlencode("q2-new-smtp-password")
        ),
    )
    .await;
    let email_confirm_path = location(&email_resp);
    assert!(
        email_confirm_path.starts_with("/admin/settings/email/confirm"),
        "expected a redirect to the email confirm page, got {email_confirm_path:?}"
    );

    // `/admin/users/{id}/recovery-link-confirm`'s own GET needs the admin
    // to hold a second factor, fresh (RFC 103 D6) — enrol and step up now
    // (the same shape `n2b_account_menu.rs`'s own version of this uses),
    // so every route in `concrete_console_routes` is reachable with a
    // plain GET.
    let secret = b"q2-seed-totp-secret\x00\x00\x00\x00\x00\x00";
    sui_id_store::repos::user_totp::upsert_pending(&state.db, admin.id, secret)
        .await
        .expect("upsert pending totp");
    sui_id_store::repos::user_totp::confirm_with_recovery(&state.db, admin.id, b"[]")
        .await
        .expect("confirm totp");
    let step_up_csrf = fetch_csrf(state, &session).await;
    let step = chrono::Utc::now().timestamp() / 30;
    let code = sui_id_core::totp::code_for_step(secret, step).await;
    let step_up_resp = post_form(
        state,
        "/me/security/step-up",
        &session,
        &step_up_csrf,
        &format!("_csrf={step_up_csrf}&code={code}&return_to=/admin"),
    )
    .await;
    assert!(
        step_up_resp.status().is_redirection(),
        "expected the step-up POST to redirect, got {}",
        step_up_resp.status()
    );

    Seed {
        session,
        client_id,
        target_user: target.id,
        non_admin_session,
        signing_key_id,
        disabled_provider_id,
        enabled_provider_id,
        linked_user: linked_user.id,
        email_confirm_path,
        email_csrf,
    }
}

/// One row per `/admin` GET route this dispatch's table (§1) covers.
/// `None` means the route renders no console page at all — each such
/// row is asserted separately, in `every_admin_get_route_belongs_to_exactly_one_group`.
fn expected_current_for(path_template: &str) -> Option<&'static str> {
    match path_template {
        "/admin" => Some("/admin"),
        "/admin/users" => Some("/admin/users"),
        "/admin/users/new" => Some("/admin/users"),
        "/admin/users/{id}" => Some("/admin/users"),
        "/admin/users/{id}/disable-confirm" => Some("/admin/users"),
        "/admin/users/{id}/delete-confirm" => Some("/admin/users"),
        "/admin/users/{id}/mfa-reset-confirm" => Some("/admin/users"),
        "/admin/users/{id}/recovery-link-confirm" => Some("/admin/users"),
        "/admin/users/{user_id}/federation/{provider_id}/unlink-confirm" => Some("/admin/users"),
        "/admin/clients" => Some("/admin/clients"),
        "/admin/clients/new" => Some("/admin/clients"),
        "/admin/clients/{id}/edit" => Some("/admin/clients"),
        "/admin/clients/{id}/delete-confirm" => Some("/admin/clients"),
        "/admin/signing-keys" => Some("/admin/signing-keys"),
        "/admin/signing-keys/rotate-confirm" => Some("/admin/signing-keys"),
        "/admin/signing-keys/{id}/delete-confirm" => Some("/admin/signing-keys"),
        "/admin/federation-providers" => Some("/admin/federation-providers"),
        "/admin/federation-providers/{id}/enable-confirm" => Some("/admin/federation-providers"),
        "/admin/federation-providers/{id}/disable-confirm" => Some("/admin/federation-providers"),
        "/admin/federation-providers/{id}/delete-confirm" => Some("/admin/federation-providers"),
        "/admin/settings/basic" => Some("/admin/settings/basic"),
        "/admin/settings/security" => Some("/admin/settings/security"),
        "/admin/audit" => Some("/admin/audit"),
        "/admin/settings/authentication" => Some("/admin/settings/authentication"),
        "/admin/settings/logs" => Some("/admin/settings/logs"),
        "/admin/settings/other" => Some("/admin/settings/other"),
        "/admin/settings/email" => Some("/admin/settings/email"),
        "/admin/settings/email/confirm" => Some("/admin/settings/email"),
        // Excluded: render no console page. Each reason, and its own
        // assertion, is in `every_admin_get_route_belongs_to_exactly_one_group`.
        "/admin/login" => None,
        "/admin/login/mfa" => None,
        "/admin/profile" => None,
        "/admin/settings" => None,
        "/admin/audit.csv" => None,
        other => panic!(
            "no row for derived route {other} — a route was added or renamed; \
             add it to expected_current_for (and to §1's table if it is new)"
        ),
    }
}

/// Concrete URLs for every row `expected_current_for` returns `Some` for,
/// substituting the seed's own ids for every `{...}` placeholder.
fn concrete_console_routes(seed: &Seed) -> Vec<(String, &'static str)> {
    let rows: &[(&str, &str)] = &[
        ("/admin", "/admin"),
        ("/admin/users", "/admin/users"),
        ("/admin/users/new", "/admin/users"),
        ("/admin/users/{id}", "/admin/users"),
        ("/admin/users/{id}/disable-confirm", "/admin/users"),
        ("/admin/users/{id}/delete-confirm", "/admin/users"),
        ("/admin/users/{id}/mfa-reset-confirm", "/admin/users"),
        ("/admin/clients", "/admin/clients"),
        ("/admin/clients/new", "/admin/clients"),
        // {id}/edit and {id}/delete-confirm need the seeded client's own
        // id, not the target user's — pushed separately, below.
        ("/admin/signing-keys", "/admin/signing-keys"),
        ("/admin/signing-keys/rotate-confirm", "/admin/signing-keys"),
        (
            "/admin/signing-keys/{id}/delete-confirm",
            "/admin/signing-keys",
        ),
        ("/admin/federation-providers", "/admin/federation-providers"),
        (
            "/admin/federation-providers/{disabled_id}/enable-confirm",
            "/admin/federation-providers",
        ),
        (
            "/admin/federation-providers/{enabled_id}/disable-confirm",
            "/admin/federation-providers",
        ),
        (
            "/admin/federation-providers/{enabled_id}/delete-confirm",
            "/admin/federation-providers",
        ),
        ("/admin/audit", "/admin/audit"),
        ("/admin/settings/basic", "/admin/settings/basic"),
        ("/admin/settings/security", "/admin/settings/security"),
        (
            "/admin/settings/authentication",
            "/admin/settings/authentication",
        ),
        ("/admin/settings/logs", "/admin/settings/logs"),
        ("/admin/settings/other", "/admin/settings/other"),
        ("/admin/settings/email", "/admin/settings/email"),
    ];
    let mut out: Vec<(String, &'static str)> = rows
        .iter()
        .map(|(tpl, item)| {
            let concrete = tpl
                .replace("{id}", &seed.target_user.to_string())
                .replace("{disabled_id}", &seed.disabled_provider_id.to_string())
                .replace("{enabled_id}", &seed.enabled_provider_id.to_string());
            (concrete, *item)
        })
        .collect();
    // Routes whose `{id}` is not the target user.
    out.push((
        format!("/admin/clients/{}/edit", seed.client_id),
        "/admin/clients",
    ));
    out.push((
        format!("/admin/clients/{}/delete-confirm", seed.client_id),
        "/admin/clients",
    ));
    out.push((
        format!("/admin/signing-keys/{}/delete-confirm", seed.signing_key_id),
        "/admin/signing-keys",
    ));
    out.push((seed.email_confirm_path.clone(), "/admin/settings/email"));
    out.push((
        format!("/admin/users/{}/recovery-link-confirm", seed.target_user),
        "/admin/users",
    ));
    out.push((
        format!(
            "/admin/users/{}/federation/{}/unlink-confirm",
            seed.linked_user, seed.enabled_provider_id
        ),
        "/admin/users",
    ));
    out
}

/// Test 1: every admin GET page belongs to exactly one group.
#[tokio::test]
async fn every_admin_get_route_belongs_to_exactly_one_group() {
    let state = test_app();
    let seed = seed(&state).await;

    // ---- Coverage: every derived route has a row. ----
    let derived: Vec<String> = routes()
        .into_iter()
        .filter(|(m, p, _)| m == "GET" && p.starts_with("/admin"))
        .map(|(_, p, _)| p)
        .collect();
    assert!(
        derived.len() >= 30,
        "the router parse found only {} /admin GET routes — the parser may have \
         changed shape",
        derived.len()
    );
    for path in &derived {
        // Panics (via expected_current_for) naming the route if it has no row.
        let _ = expected_current_for(path);
    }

    // ---- The excluded routes: each asserted for its own reason. ----
    let login_resp = get(&state, "/admin/login", &seed.session).await;
    assert!(
        login_resp.status().is_redirection(),
        "an already-signed-in admin GETting /admin/login: expected a redirect, got {}",
        login_resp.status()
    );

    let mfa_resp = get(&state, "/admin/login/mfa", &seed.session).await;
    assert_eq!(mfa_resp.status(), StatusCode::OK);
    let mfa_html = html_of(mfa_resp).await;
    assert!(
        !body_only(&mfa_html).contains("app-sidebar"),
        "/admin/login/mfa: renders AuthShell, not the console — must carry no sidebar"
    );

    let profile_resp = get(&state, "/admin/profile", &seed.session).await;
    assert!(profile_resp.status().is_redirection());
    assert_eq!(location(&profile_resp), "/me/security/overview");

    let settings_resp = get(&state, "/admin/settings", &seed.session).await;
    assert!(settings_resp.status().is_redirection());
    assert_eq!(location(&settings_resp), "/admin/settings/basic");

    let csv_resp = get(&state, "/admin/audit.csv", &seed.session).await;
    assert_eq!(csv_resp.status(), StatusCode::OK);
    let ct = csv_resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.contains("csv"),
        "/admin/audit.csv: content-type was {ct:?}"
    );

    // ---- Every console page: exactly one item marked current, the right one. ----
    for (uri, item_href) in concrete_console_routes(&seed) {
        let resp = get(&state, &uri, &seed.session).await;
        assert_eq!(resp.status(), StatusCode::OK, "{uri}: expected 200");
        let html = html_of(resp).await;
        assert_exactly_current(&html, item_href, &uri);
    }
}

/// Test 2: the pages a `POST` renders inside the console.
///
/// Found by grepping every `admin`/`settings` POST handler for a direct
/// `render_*`/`Html(render_*)` call in its body (not only a `Redirect`):
/// `grep -n "pub async fn.*_post\|_create\|_rotate\|_test\|recovery_link" crates/sui-id/src/http/handlers/admin/*.rs
/// crates/sui-id/src/http/handlers/settings.rs`, then reading each match.
/// Six unconditionally- or success-path-rendered results were found (the
/// list the dispatch's examples point at); several more only render on a
/// validation-failure redisplay and are exercised by their own existing
/// tests elsewhere, not repeated here.
#[tokio::test]
async fn posts_that_render_inside_the_console() {
    let state = test_app();
    let seed = seed(&state).await;

    // `clients_rotate_secret_post` → `render_client_edit` (Apps).
    let csrf = fetch_csrf(&state, &seed.session).await;
    let resp = post_form(
        &state,
        &format!("/admin/clients/{}/rotate-secret", seed.client_id),
        &seed.session,
        &csrf,
        &format!("_csrf={csrf}&_confirmed=1"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_exactly_current(
        &html_of(resp).await,
        "/admin/clients",
        "POST .../rotate-secret",
    );

    // `federation_providers_check_post` → `render_confirm_enable_federation_provider`
    // (Sign-in → Federation providers). Always renders; never a redirect.
    let csrf = fetch_csrf(&state, &seed.session).await;
    let resp = post_form(
        &state,
        &format!(
            "/admin/federation-providers/{}/check",
            seed.disabled_provider_id
        ),
        &seed.session,
        &csrf,
        &format!("_csrf={csrf}"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_exactly_current(
        &html_of(resp).await,
        "/admin/federation-providers",
        "POST .../check",
    );

    // `email_test` → `render_settings_email` with a flash (Email). Always
    // renders the settings/email page back, never a redirect.
    let csrf = fetch_csrf(&state, &seed.session).await;
    let resp = post_form(
        &state,
        "/admin/settings/email/test",
        &seed.session,
        &csrf,
        &format!("_csrf={csrf}"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_exactly_current(
        &html_of(resp).await,
        "/admin/settings/email",
        "POST .../email/test",
    );

    // `email_confirm_post` → `render_settings_email` with a flash (Email).
    // Reuses `seed.email_csrf`, not a freshly fetched token: the pending
    // change is bound to the token active when it was created (RFC 090
    // P5), and a fresh `fetch_csrf` call rotates the cookie.
    let pending_id = seed
        .email_confirm_path
        .split("pending_change_id=")
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    let resp = post_form(
        &state,
        "/admin/settings/email/confirm",
        &seed.session,
        &seed.email_csrf,
        &format!("_csrf={}&pending_change_id={pending_id}", seed.email_csrf),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_exactly_current(
        &html_of(resp).await,
        "/admin/settings/email",
        "POST /admin/settings/email/confirm",
    );

    // `basic_lang_post` → `render_settings_basic` with a flash (General).
    let csrf = fetch_csrf(&state, &seed.session).await;
    let resp = post_form(
        &state,
        "/admin/settings/basic/lang",
        &seed.session,
        &csrf,
        &format!("_csrf={csrf}&default_lang=en"),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_exactly_current(
        &html_of(resp).await,
        "/admin/settings/basic",
        "POST .../basic/lang",
    );

    // `users_recovery_link` (success path) → `render_recovery_link_issued`
    // (Users). `seed()` already enrolled the admin's TOTP and stepped up
    // fresh (RFC 103 D6), for this and for `recovery-link-confirm`'s own
    // GET.
    let csrf = fetch_csrf(&state, &seed.session).await;
    let resp = post_form(
        &state,
        &format!("/admin/users/{}/recovery-link", seed.target_user),
        &seed.session,
        &csrf,
        &format!(
            "_csrf={csrf}&_confirmed=1&reason={}",
            urlencode("q2 test 2, issued path")
        ),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_exactly_current(
        &html_of(resp).await,
        "/admin/users",
        "POST .../recovery-link (issued)",
    );
}

/// Every `<p class="app-sidebar__group">` and `<a class="app-sidebar__link"
/// href="...">` in `html`, as an ordered token stream: `Ok(href)` for a
/// link, `Err(())` for a group boundary. Locale-independent (reads
/// structure, not label text — this test environment's default locale is
/// Japanese, and the labels in §1's table are English).
fn sidebar_tokens(html: &str) -> Vec<Result<String, ()>> {
    let mut out = Vec::new();
    let mut from = 0usize;
    loop {
        let next_group = html[from..].find(r#"<p class="app-sidebar__group">"#);
        let next_link = html[from..].find(r#"<a href=""#);
        match (next_group, next_link) {
            (None, None) => break,
            (Some(g), Some(l)) if g < l => {
                out.push(Err(()));
                from += g + 1;
            }
            (Some(_), Some(l)) => {
                let hs = from + l + r#"<a href=""#.len();
                let he = html[hs..].find('"').map(|e| hs + e).expect("closing quote");
                out.push(Ok(html[hs..he].to_owned()));
                from = he;
            }
            (Some(g), None) => {
                out.push(Err(()));
                from += g + 1;
            }
            (None, Some(l)) => {
                let hs = from + l + r#"<a href=""#.len();
                let he = html[hs..].find('"').map(|e| hs + e).expect("closing quote");
                out.push(Ok(html[hs..he].to_owned()));
                from = he;
            }
        }
    }
    out
}

/// Test 3: the rendered sidebar's links equal `NavItem::ALL`, in order,
/// under the six group headings, in order.
#[tokio::test]
async fn the_sidebar_is_the_list() {
    let state = test_app();
    let seed = seed(&state).await;
    let html = html_of(get(&state, "/admin", &seed.session).await).await;

    // Scope to the wide sidebar copy only (`.app-sidebar--wide`): the
    // narrow copy repeats the identical content, which would otherwise
    // double every match below.
    let start = html
        .find(r#"class="app-sidebar app-sidebar--wide""#)
        .expect("wide sidebar present");
    let end = html[start..]
        .find(r#"class="app-sidebar app-sidebar--narrow""#)
        .map(|e| start + e)
        .expect("narrow sidebar present after the wide one");
    // Stop before the account menu: its own links (`/me/security/...`)
    // are not sidebar nav items.
    let account_start = html[start..end]
        .find(r#"class="app-sidebar__account""#)
        .map(|e| start + e)
        .unwrap_or(end);
    let sidebar = &html[start..account_start];
    let tokens = sidebar_tokens(sidebar);

    // Err = a group boundary, Ok(href) = an item under the group just
    // opened. §1's table, in order.
    let expected: Vec<Result<&str, ()>> = vec![
        Err(()),
        Ok("/admin"),
        Err(()),
        Ok("/admin/users"),
        Err(()),
        Ok("/admin/clients"),
        Err(()),
        Ok("/admin/federation-providers"),
        Ok("/admin/settings/authentication"),
        Err(()),
        Ok("/admin/signing-keys"),
        Ok("/admin/settings/security"),
        Ok("/admin/audit"),
        Err(()),
        Ok("/admin/settings/basic"),
        Ok("/admin/settings/email"),
        Ok("/admin/settings/logs"),
        Ok("/admin/settings/other"),
    ];
    let got: Vec<Result<&str, ()>> = tokens
        .iter()
        .map(|t| match t {
            Ok(h) => Ok(h.as_str()),
            Err(()) => Err(()),
        })
        .collect();
    assert_eq!(
        got, expected,
        "the sidebar's groups/items do not match §1's table, in order"
    );
}

/// Test 4 — RFC 151's closure evidence: the password minimum is found
/// under *Sign-in*, on `/admin/settings/authentication`.
#[tokio::test]
async fn rfc151_closure_the_minimum_length_is_under_sign_in() {
    let state = test_app();
    let seed = seed(&state).await;
    let html = html_of(get(&state, "/admin/settings/authentication", &seed.session).await).await;
    assert_exactly_current(
        &html,
        "/admin/settings/authentication",
        "/admin/settings/authentication",
    );
    // "Under Sign-in": the same group as Federation providers (§1's
    // table), checked structurally rather than by the group's English
    // label text (this test environment's default locale is Japanese).
    let start = html
        .find(r#"class="app-sidebar app-sidebar--wide""#)
        .expect("wide sidebar present");
    let end = html[start..]
        .find(r#"class="app-sidebar app-sidebar--narrow""#)
        .map(|e| start + e)
        .expect("narrow sidebar present");
    let tokens = sidebar_tokens(&html[start..end]);
    let group_of = |href: &str| -> usize {
        let mut group = 0usize;
        for t in &tokens {
            match t {
                Err(()) => group += 1,
                Ok(h) if h == href => return group,
                Ok(_) => {}
            }
        }
        panic!("{href}: not found in the sidebar")
    };
    assert_eq!(
        group_of("/admin/settings/authentication"),
        group_of("/admin/federation-providers"),
        "Sign-in settings is not in the same group as Federation providers"
    );
    assert!(
        html.contains("password_min_len"),
        "the minimum-length form field is not on this page"
    );
}

/// Test 5: the account menu's contents, with both chromes, and its
/// absence on every signed-out page.
#[tokio::test]
async fn the_account_menu_has_its_four_items_and_is_absent_signed_out() {
    let state = test_app();
    let seed = seed(&state).await;

    let console_html = html_of(get(&state, "/admin", &seed.session).await).await;
    for want in [
        r#"href="/me/security/overview""#,
        r#"href="/me/security/language""#,
        "data-theme-value=\"light\"",
        "data-theme-value=\"system\"",
        "data-theme-value=\"dark\"",
        r#"action="/admin/logout""#,
    ] {
        assert!(
            console_html.contains(want),
            "console chrome's account menu is missing {want:?}"
        );
    }

    let account_html =
        html_of(get(&state, "/me/security/overview", &seed.non_admin_session).await).await;
    for want in [
        r#"href="/me/security/overview""#,
        r#"href="/me/security/language""#,
        "data-theme-value=\"light\"",
        r#"action="/admin/logout""#,
    ] {
        assert!(
            account_html.contains(want),
            "account chrome's account menu is missing {want:?}"
        );
    }

    // Signed out: no session cookie at all.
    let signed_out = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/admin/login")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    let signed_out_html = html_of(signed_out).await;
    assert!(
        !signed_out_html.contains("data-theme-value"),
        "the sign-in page must show no theme selector"
    );
    assert!(
        !signed_out_html.contains(r#"class="user-menu""#),
        "the sign-in page must show no account menu"
    );

    // Sign-out still signs out.
    let csrf = fetch_csrf(&state, &seed.session).await;
    // `admin::logout` renders the login page directly (200, with a
    // "signed out" confirmation), not a redirect.
    let logout_resp = post_form(
        &state,
        "/admin/logout",
        &seed.session,
        &csrf,
        &format!("_csrf={csrf}"),
    )
    .await;
    assert_eq!(logout_resp.status(), StatusCode::OK);
    let after_logout = get(&state, "/admin", &seed.session).await;
    assert!(
        after_logout.status().is_redirection(),
        "the session must no longer be valid after sign-out"
    );
}

/// Test 6: the version appears exactly where §4 says, and nowhere else.
#[tokio::test]
async fn the_version_is_console_only() {
    let state = test_app();
    let seed = seed(&state).await;
    let marker = format!("sui-id v{}", env!("CARGO_PKG_VERSION"));

    let console_html = html_of(get(&state, "/admin", &seed.session).await).await;
    // The sidebar renders twice (one copy per breakpoint, RFC 149 §2), so
    // the version appears twice in the markup — once per copy — even
    // though only one copy is ever visible. Scope to the wide copy alone.
    let start = console_html
        .find(r#"class="app-sidebar app-sidebar--wide""#)
        .expect("wide sidebar present");
    let end = console_html[start..]
        .find(r#"class="app-sidebar app-sidebar--narrow""#)
        .map(|e| start + e)
        .expect("narrow sidebar present");
    assert_eq!(
        console_html[start..end].matches(&marker).count(),
        1,
        "the console chrome's wide sidebar must show the version exactly once"
    );

    let account_html =
        html_of(get(&state, "/me/security/overview", &seed.non_admin_session).await).await;
    assert!(
        !account_html.contains(&marker),
        "the account chrome must show no version"
    );

    let signed_out_html = html_of(
        build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/admin/login")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("GET"),
    )
    .await;
    assert!(
        !signed_out_html.contains(&marker),
        "the sign-in page must show no version"
    );

    let setup_html = html_of(
        build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/setup")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("GET"),
    )
    .await;
    assert!(
        !setup_html.contains(&marker),
        "a setup page must show no version"
    );
}

/// Test 7: exactly one `<h1>` per page, over every page test 1 fetches
/// plus the sign-in and setup pages.
#[tokio::test]
async fn exactly_one_h1_per_page() {
    // A fresh, not-yet-initialized state for /setup: once `seed()` below
    // completes setup, /setup redirects away instead of rendering.
    let fresh_state = test_app();
    let setup_resp = build_router(fresh_state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/setup")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("GET");
    assert_eq!(setup_resp.status(), StatusCode::OK, "/setup: expected 200");
    let setup_html = html_of(setup_resp).await;
    assert_eq!(
        body_only(&setup_html).matches("<h1").count(),
        1,
        "/setup: expected exactly one <h1>"
    );

    let state = test_app();
    let seed = seed(&state).await;

    let mut uris: Vec<String> = concrete_console_routes(&seed)
        .into_iter()
        .map(|(u, _)| u)
        .collect();
    uris.push("/admin/login".into());

    for uri in uris {
        let resp = if uri == "/admin/login" {
            build_router(state.clone())
                .oneshot(
                    Request::builder()
                        .method(Method::GET)
                        .uri(uri.as_str())
                        .body(Body::empty())
                        .expect("req"),
                )
                .await
                .expect("GET")
        } else {
            get(&state, &uri, &seed.session).await
        };
        assert_eq!(resp.status(), StatusCode::OK, "{uri}: expected 200");
        let html = html_of(resp).await;
        let body = body_only(&html);
        assert_eq!(
            body.matches("<h1").count(),
            1,
            "{uri}: expected exactly one <h1>, found {}",
            body.matches("<h1").count()
        );
    }
}

/// Test 8: no `title=` attribute and no `role="menu"` in the chrome.
#[tokio::test]
async fn chrome_has_no_title_attribute_and_no_menu_role() {
    let state = test_app();
    let seed = seed(&state).await;

    let console_html = html_of(get(&state, "/admin", &seed.session).await).await;
    let console_body = body_only(&console_html);
    assert!(
        !console_body.contains(" title=\""),
        "console chrome has a title= attribute"
    );
    assert!(
        !console_body.contains(r#"role="menu""#),
        "console chrome has role=\"menu\""
    );
    assert!(
        !console_body.contains(r#"role="menuitem""#),
        "console chrome has role=\"menuitem\""
    );

    let signed_out_html = html_of(
        build_router(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/admin/login")
                    .body(Body::empty())
                    .expect("req"),
            )
            .await
            .expect("GET"),
    )
    .await;
    let signed_out_html = body_only(&signed_out_html).to_owned();
    assert!(
        !signed_out_html.contains(" title=\""),
        "AuthShell (sign-in) has a title= attribute"
    );
    assert!(
        !signed_out_html.contains(r#"role="menu""#),
        "AuthShell (sign-in) has role=\"menu\""
    );
}
