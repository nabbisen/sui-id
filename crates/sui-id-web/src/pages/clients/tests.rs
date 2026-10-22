//! RFC 136 D3 (step 2): the test that holds the self-registered marker.
//! RFC 095 M3 stage 4a §5 extends this file with the auth-method-unrecorded
//! marker, on the same two render sites, same shape.
//!
//! Each marker — `(registered_via == "dynamic")` and
//! `(token_endpoint_auth_method.is_none())` — is written out independently
//! at both render sites: the list row (`client_row_view`, inlined into
//! [`render_clients`]) and the edit view ([`render_client_edit`]). Each site
//! is asserted on both of its values, so a change to one that silently
//! disagrees with the other fails here.
//!
//! Asserted against the i18n value each badge renders — not the English
//! literal, which a translation change would be free to touch.

use super::*;
use sui_id_shared::ids::ClientId;

const LANG: sui_id_i18n::Locale = sui_id_i18n::Locale::En;

fn marker() -> &'static str {
    LANG.strings().status_self_registered
}

fn unrecorded_marker() -> &'static str {
    LANG.strings().status_auth_method_unrecorded
}

/// `auth_method` is `None` for "never recorded"; `Some(_)` for "recorded as
/// this wire value" — the actual value never affects either test here, only
/// its presence, so every "recorded" test uses the same placeholder.
fn client_summary(registered_via: &str, auth_method: Option<&str>) -> ClientSummary {
    ClientSummary {
        id: ClientId::new(),
        name: "a client".into(),
        redirect_uris: vec!["https://example.com/cb".into()],
        allowed_scopes: String::new(),
        post_logout_redirect_uris: Vec::new(),
        confidential: true,
        is_disabled: false,
        is_deleted: false,
        consent_policy: "first_time".into(),
        registered_via: registered_via.into(),
        token_endpoint_auth_method: auth_method.map(str::to_string),
        auth_method_unrecognized: false,
        created_at: chrono::Utc::now(),
    }
}

fn client_edit_data(registered_via: &str, auth_method: Option<&str>) -> ClientEditData {
    ClientEditData {
        id: "11111111-1111-1111-1111-111111111111".into(),
        name: "a client".into(),
        redirect_uris: vec!["https://example.com/cb".into()],
        allowed_scopes: String::new(),
        post_logout_redirect_uris: Vec::new(),
        confidential: true,
        is_disabled: false,
        consent_policy: "first_time".into(),
        registered_via: registered_via.into(),
        token_endpoint_auth_method: auth_method.map(str::to_string),
        auth_method_unrecognized: false,
        freshly_rotated_secret: None,
    }
}

#[test]
fn render_clients_marks_a_dynamically_registered_client() {
    let html = render_clients(
        true,
        vec![client_summary("dynamic", Some("client_secret_basic"))],
        None,
        None,
        "csrf".into(),
        false,
        LANG,
    );
    assert!(
        html.contains(marker()),
        "a dynamically registered client must carry the self-registered marker"
    );
}

#[test]
fn render_clients_does_not_mark_an_admin_created_client() {
    let html = render_clients(
        true,
        vec![client_summary("admin", Some("client_secret_basic"))],
        None,
        None,
        "csrf".into(),
        false,
        LANG,
    );
    assert!(
        !html.contains(marker()),
        "an administrator-created client must not carry the self-registered marker"
    );
}

#[test]
fn render_client_edit_marks_a_dynamically_registered_client() {
    let html = render_client_edit(
        true,
        client_edit_data("dynamic", Some("client_secret_basic")),
        None,
        "csrf".into(),
        LANG,
    );
    assert!(
        html.contains(marker()),
        "the edit view must carry the marker for a dynamically registered client"
    );
}

#[test]
fn render_client_edit_does_not_mark_an_admin_created_client() {
    let html = render_client_edit(
        true,
        client_edit_data("admin", Some("client_secret_basic")),
        None,
        "csrf".into(),
        LANG,
    );
    assert!(
        !html.contains(marker()),
        "the edit view must not carry the marker for an administrator-created client"
    );
}

// ---------- RFC 095 M3 stage 4a §5: the auth-method-unrecorded marker ----------

#[test]
fn render_clients_marks_a_client_with_no_recorded_auth_method() {
    let html = render_clients(
        true,
        vec![client_summary("admin", None)],
        None,
        None,
        "csrf".into(),
        false,
        LANG,
    );
    assert!(
        html.contains(unrecorded_marker()),
        "a client with no recorded token_endpoint_auth_method must carry the \
         unrecorded marker"
    );
}

#[test]
fn render_clients_does_not_mark_a_client_with_a_recorded_auth_method() {
    let html = render_clients(
        true,
        vec![client_summary("admin", Some("client_secret_basic"))],
        None,
        None,
        "csrf".into(),
        false,
        LANG,
    );
    assert!(
        !html.contains(unrecorded_marker()),
        "a client with a recorded token_endpoint_auth_method must not be \
         rendered as unrecorded"
    );
}

#[test]
fn render_client_edit_marks_a_client_with_no_recorded_auth_method() {
    let html = render_client_edit(
        true,
        client_edit_data("admin", None),
        None,
        "csrf".into(),
        LANG,
    );
    assert!(
        html.contains(unrecorded_marker()),
        "the edit view must carry the unrecorded marker when the auth method \
         was never recorded"
    );
}

#[test]
fn render_client_edit_does_not_mark_a_client_with_a_recorded_auth_method() {
    let html = render_client_edit(
        true,
        client_edit_data("admin", Some("client_secret_basic")),
        None,
        "csrf".into(),
        LANG,
    );
    assert!(
        !html.contains(unrecorded_marker()),
        "the edit view must not render a recorded auth method as unrecorded"
    );
}

// ---------- RFC 095 M3 stage 4b §0.2: the auth-method-unrecognized marker ----------

fn unrecognized_marker() -> &'static str {
    LANG.strings().status_auth_method_unrecognized
}

#[test]
fn render_clients_marks_a_client_with_an_unrecognized_auth_method() {
    let mut summary = client_summary("admin", Some("CLIENT_SECRET_BASIC"));
    summary.auth_method_unrecognized = true;
    let html = render_clients(true, vec![summary], None, None, "csrf".into(), false, LANG);
    assert!(
        html.contains(unrecognized_marker()),
        "a client whose stored auth method is unparseable must carry the \
         unrecognized marker, distinct from \"unrecorded\""
    );
    assert!(
        !html.contains(unrecorded_marker()),
        "an unrecognized value is present, not absent -- it must not also \
         read as unrecorded"
    );
}

#[test]
fn render_client_edit_marks_a_client_with_an_unrecognized_auth_method() {
    let mut data = client_edit_data("admin", Some("CLIENT_SECRET_BASIC"));
    data.auth_method_unrecognized = true;
    let html = render_client_edit(true, data, None, "csrf".into(), LANG);
    assert!(
        html.contains(unrecognized_marker()),
        "the edit view must carry the unrecognized marker when the stored \
         value is unparseable"
    );
    assert!(!html.contains(unrecorded_marker()));
}
