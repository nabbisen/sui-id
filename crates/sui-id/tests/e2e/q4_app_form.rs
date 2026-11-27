//! RFC 149 Q4 (`0.95.0-q4-the-app-form`), §5: the app form — scope
//! checkboxes generated from the one list discovery advertises,
//! additional-scope near-miss refusal, one input per redirect/post-
//! logout URI with no-JS add/remove, and C01 recording the consent
//! policy chosen at creation. Tests 1, 2, 4, 5, 6 and 7 of that
//! dispatch's §5 table live here. Test 3 (the checkboxes are the
//! discovery list) and test 8 (create and edit share one component)
//! are unit tests over the rendered markup instead
//! (`pages::clients::tests` in `sui-id-web`, G21) — no HTTP round
//! trip needed for either.

use super::common::*;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use std::str::FromStr;
use sui_id::{AppState, build_router};
use sui_id_shared::ids::ClientId;
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

async fn html_of(resp: axum::response::Response) -> String {
    let body = read_body(resp.into_body()).await;
    String::from_utf8_lossy(&body).to_string()
}

async fn client_count(state: &AppState) -> usize {
    sui_id_store::repos::clients::list(&state.db)
        .await
        .expect("list clients")
        .len()
}

async fn audit_count(state: &AppState, action: &str) -> i64 {
    let action = action.to_owned();
    state
        .db
        .with_conn(move |c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM audit_log WHERE action = ?1",
                [&action],
                |row| row.get::<_, i64>(0),
            )?)
        })
        .await
        .expect("count")
}

/// Seed a client directly with a stored `allowed_scopes` that a
/// near-miss slipped into before this release (old field had no
/// such check) -- the shape §5 test 6 asks for, and the only way to
/// reach it now that the form itself refuses one.
async fn seed_client_with_scopes(state: &AppState, name: &str, allowed_scopes: &str) -> String {
    let admin = sui_id_store::repos::users::find_by_username(&state.db, USERNAME)
        .await
        .expect("admin");
    // `create_client` would itself refuse/reshape this; write the row
    // directly, the same way a pre-this-release stored value would
    // have arrived.
    let row = sui_id_store::models::ClientRow {
        id: sui_id_shared::ids::ClientId::new(),
        name: name.to_owned(),
        confidential: false,
        secret_hash: None,
        redirect_uris: vec!["https://rp.test/cb".into()],
        allowed_scopes: allowed_scopes.to_owned(),
        post_logout_redirect_uris: vec![],
        is_disabled: false,
        is_deleted: false,
        consent_policy: sui_id_store::models::ConsentPolicy::default(),
        registered_via: sui_id_store::models::RegistrationSource::Admin,
        logo_uri: None,
        homepage_uri: None,
        privacy_policy_uri: None,
        tos_uri: None,
        token_endpoint_auth_method: None,
        grant_types: None,
        response_types: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let id = row.id;
    sui_id_store::commands::create_client(&state.db, admin.id, row)
        .await
        .expect("seed client");
    id.to_string()
}

// ---------- test 1: Add another / Remove save and record nothing ----------

#[tokio::test]
async fn add_another_and_remove_save_nothing_and_record_nothing() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let csrf = fetch_csrf(&state, &session).await;
    let before_clients = client_count(&state).await;
    let before_audit = audit_count(&state, "client.create").await;

    let body = format!(
        "name=my-app&redirect_uri_0=https%3A%2F%2Frp.test%2Fcb&add_redirect_uri=1&_csrf={csrf}"
    );
    let resp = post_form(&state, "/admin/clients", &session, &csrf, &body).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "Add another must re-render, not redirect"
    );
    let html = html_of(resp).await;
    assert!(
        html.contains("name=\"redirect_uri_0\"") && html.contains("name=\"redirect_uri_1\""),
        "one more row must appear: {html}"
    );
    assert!(
        html.contains("value=\"https://rp.test/cb\""),
        "the already-typed URI must be kept"
    );
    assert_eq!(
        client_count(&state).await,
        before_clients,
        "nothing created"
    );
    assert_eq!(
        audit_count(&state, "client.create").await,
        before_audit,
        "nothing recorded"
    );

    // Now remove the row just added.
    let csrf2 = fetch_csrf(&state, &session).await;
    let body = format!(
        "name=my-app&redirect_uri_0=https%3A%2F%2Frp.test%2Fcb&redirect_uri_1=&remove_redirect_uri=1&_csrf={csrf2}"
    );
    let resp = post_form(&state, "/admin/clients", &session, &csrf2, &body).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let html = html_of(resp).await;
    assert!(
        html.contains("name=\"redirect_uri_0\"") && !html.contains("name=\"redirect_uri_1\""),
        "the removed row must be gone: {html}"
    );
    assert_eq!(
        client_count(&state).await,
        before_clients,
        "still nothing created"
    );
    assert_eq!(
        audit_count(&state, "client.create").await,
        before_audit,
        "still nothing recorded"
    );
}

// ---------- test 2: each URI is checked on its own ----------

#[tokio::test]
async fn each_redirect_uri_is_checked_on_its_own() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let csrf = fetch_csrf(&state, &session).await;

    let body = format!(
        "name=my-app&redirect_uri_0=https%3A%2F%2Frp.test%2Fcb\
         &redirect_uri_1=not-a-url&save=1&_csrf={csrf}"
    );
    let resp = post_form(&state, "/admin/clients", &session, &csrf, &body).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let html = html_of(resp).await;
    assert!(
        html.contains("value=\"https://rp.test/cb\""),
        "the first, valid URI must be kept: {html}"
    );
    assert!(
        html.contains("name=\"redirect_uri_1\" aria-label=\"Redirect URI 2\""),
        "the second row must still be named by its own position: {html}"
    );
    let ja = sui_id_i18n::Locale::Ja.strings();
    assert!(
        html.contains(ja.clients_error_redirect_uri_not_url),
        "the second URI's own error must appear: {html}"
    );
}

// ---------- test 4: openid is always saved ----------

#[tokio::test]
async fn openid_is_always_saved_even_when_omitted() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let csrf = fetch_csrf(&state, &session).await;

    // No scope_* field at all -- every checkbox unchecked.
    let body =
        format!("name=no-scopes-app&redirect_uri_0=https%3A%2F%2Frp.test%2Fcb&save=1&_csrf={csrf}");
    let resp = post_form(&state, "/admin/clients", &session, &csrf, &body).await;
    assert_eq!(resp.status(), StatusCode::OK, "expected success");

    let rows = sui_id_store::repos::clients::list(&state.db)
        .await
        .expect("list");
    let created = rows
        .iter()
        .find(|r| r.name == "no-scopes-app")
        .expect("the just-created client");
    assert_eq!(created.allowed_scopes, "openid");
}

// ---------- test 5: near misses ----------

#[tokio::test]
async fn near_miss_additional_scopes_are_refused() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let ja = sui_id_i18n::Locale::Ja.strings();

    for (typed, standard) in [
        ("profle", "profile"),
        ("emial", "email"),
        ("Profile", "profile"),
    ] {
        let csrf = fetch_csrf(&state, &session).await;
        let body = format!(
            "name=near-miss-app&redirect_uri_0=https%3A%2F%2Frp.test%2Fcb\
             &additional_scopes={typed}&save=1&_csrf={csrf}"
        );
        let resp = post_form(&state, "/admin/clients", &session, &csrf, &body).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{typed}");
        let html = html_of(resp).await;
        assert!(
            html.contains(&(ja.clients_error_additional_scope_near_miss)(
                typed, standard
            )),
            "{typed}: expected the near-miss message naming {standard}: {html}"
        );
    }

    // A standard scope typed in the additional field.
    let csrf = fetch_csrf(&state, &session).await;
    let body = format!(
        "name=near-miss-app&redirect_uri_0=https%3A%2F%2Frp.test%2Fcb\
         &additional_scopes=email&save=1&_csrf={csrf}"
    );
    let resp = post_form(&state, "/admin/clients", &session, &csrf, &body).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let html = html_of(resp).await;
    assert!(
        html.contains(&(ja.clients_error_additional_scope_is_standard)("email")),
        "{html}"
    );

    // A genuine additional scope is saved.
    let csrf = fetch_csrf(&state, &session).await;
    let body = format!(
        "name=real-additional-scope-app&redirect_uri_0=https%3A%2F%2Frp.test%2Fcb\
         &additional_scopes=my_api.read&save=1&_csrf={csrf}"
    );
    let resp = post_form(&state, "/admin/clients", &session, &csrf, &body).await;
    assert_eq!(resp.status(), StatusCode::OK, "{}", html_of(resp).await);
    let rows = sui_id_store::repos::clients::list(&state.db)
        .await
        .expect("list");
    let created = rows
        .iter()
        .find(|r| r.name == "real-additional-scope-app")
        .expect("created");
    assert_eq!(created.allowed_scopes, "openid my_api.read");

    // A token containing a `"` is refused -- RFC 6749 §3.3's syntax
    // (`%x23-5B / %x5D-7E`) excludes it, checked before the near-miss
    // distance at all. (A literal space can never reach the syntax
    // check as part of one token -- the field splits on whitespace
    // first, so "a b" is two valid tokens, not one malformed one.)
    let csrf = fetch_csrf(&state, &session).await;
    let body = format!(
        "name=bad-token-app&redirect_uri_0=https%3A%2F%2Frp.test%2Fcb\
         &additional_scopes={}&save=1&_csrf={csrf}",
        utf8_encode("has\"quote"),
    );
    let resp = post_form(&state, "/admin/clients", &session, &csrf, &body).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------- test 6: a near miss saved before this release ----------

#[tokio::test]
async fn a_stored_near_miss_shows_the_message_and_refuses_an_unchanged_save() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let id = seed_client_with_scopes(&state, "legacy-app", "openid profle").await;

    let html = html_of(get(&state, &format!("/admin/clients/{id}/edit"), &session).await).await;
    assert!(
        html.contains("value=\"profle\"") || html.contains(">profle<"),
        "the stored near miss must show under Additional scopes: {html}"
    );
    let ja = sui_id_i18n::Locale::Ja.strings();
    assert!(
        html.contains(&(ja.clients_error_additional_scope_near_miss)(
            "profle", "profile"
        )),
        "the same message must show without any POST: {html}"
    );

    // Saving it unchanged is still refused.
    let csrf = fetch_csrf(&state, &session).await;
    let body = format!(
        "name=legacy-app&redirect_uri_0=https%3A%2F%2Frp.test%2Fcb\
         &additional_scopes=profle&save=1&_csrf={csrf}"
    );
    let resp = post_form(
        &state,
        &format!("/admin/clients/{id}/edit"),
        &session,
        &csrf,
        &body,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let row = sui_id_store::repos::clients::get(&state.db, ClientId::from_str(&id).unwrap())
        .await
        .expect("get");
    assert_eq!(
        row.allowed_scopes, "openid profle",
        "nothing that worked stops working -- the stored value is untouched"
    );
}

// ---------- test 3: the checkboxes are the discovery list ----------

#[tokio::test]
async fn the_scope_checkboxes_are_the_discovery_list_in_order() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let html = html_of(get(&state, "/admin/clients/new", &session).await).await;

    // `openid` has no `name` (it is always included, never a choice);
    // the rest are checkboxes named `scope_<scope>`, in the same
    // order `sui_id_core::discovery::STANDARD_SCOPES` lists them —
    // the one list this dispatch makes discovery and the form share.
    let expected: Vec<&str> = sui_id_core::discovery::STANDARD_SCOPES
        .iter()
        .skip(1)
        .copied()
        .collect();
    let mut found = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = html[from..].find("name=\"scope_") {
        let start = from + rel + "name=\"scope_".len();
        let end = start + html[start..].find('"').expect("closing quote");
        found.push(html[start..end].to_owned());
        from = end;
    }
    assert_eq!(
        found, expected,
        "checkbox order must match the discovery list"
    );
    assert!(
        html.contains("id=\"scope-openid\"") && !html.contains("name=\"scope_openid\""),
        "openid must render with no name (it is never a submitted choice): {html}"
    );
}

// ---------- test 7: C01 records the consent policy ----------

#[tokio::test]
async fn c01_records_the_consent_policy_chosen_at_creation() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let csrf = fetch_csrf(&state, &session).await;

    let body = format!(
        "name=always-ask-app&redirect_uri_0=https%3A%2F%2Frp.test%2Fcb\
         &consent_policy=always&save=1&_csrf={csrf}"
    );
    let resp = post_form(&state, "/admin/clients", &session, &csrf, &body).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let note: Option<String> = state
        .db
        .with_conn(|c| {
            Ok(c.query_row(
                "SELECT note FROM audit_log WHERE action = 'client.create' \
                 ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get(0),
            )?)
        })
        .await
        .expect("audit row");
    let note = note.expect("note");
    assert_eq!(
        sui_id_store::registry::note_field(&note, "consent_policy").as_deref(),
        Some("always")
    );
}

/// Q4's review: an app whose stored scope list is empty permits *any*
/// scope (`enforce_scope_policy`). The checkboxes cannot show that, and
/// a save always writes a list, so the edit page must say so before an
/// unrelated edit restricts the app. It starts from the new-app default
/// (`profile` and `email`), and the stored row is untouched by viewing.
#[tokio::test]
async fn an_empty_stored_scope_list_is_named_before_a_save_can_restrict_it() {
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let id = seed_client_with_scopes(&state, "permit-any-app", "").await;

    let html = html_of(get(&state, &format!("/admin/clients/{id}/edit"), &session).await).await;
    let ja = sui_id_i18n::Locale::Ja.strings();
    assert!(
        html.contains(ja.clients_scopes_empty_notice),
        "the edit page must say the stored list is empty and permits any scope: {html}"
    );
    for scope in ["profile", "email"] {
        let marker = format!("id=\"scope-{scope}\"");
        let at = html
            .find(&marker)
            .expect("the standard checkbox is rendered");
        let tag_start = html[..at].rfind('<').expect("tag start");
        let tag_end = at + html[at..].find('>').expect("tag end");
        assert!(
            html[tag_start..tag_end].contains("checked"),
            "{scope} starts ticked, as for a new app"
        );
    }
    let row = sui_id_store::repos::clients::get(&state.db, ClientId::from_str(&id).unwrap())
        .await
        .expect("get");
    assert_eq!(row.allowed_scopes, "", "viewing the page changes nothing");

    // A list that is not empty gets no such notice.
    let other = seed_client_with_scopes(&state, "listed-app", "openid email").await;
    let html = html_of(get(&state, &format!("/admin/clients/{other}/edit"), &session).await).await;
    assert!(
        !html.contains(ja.clients_scopes_empty_notice),
        "only an empty list gets the notice"
    );
}
