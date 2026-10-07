//! RFC 136 D3 (step 2): the test that holds the self-registered marker.
//!
//! The marker — RFC 136 D2 — is `(registered_via == "dynamic")`, written out
//! independently at both render sites: the list row (`client_row_view`,
//! inlined into [`render_clients`]) and the edit view
//! ([`render_client_edit`]). Each site is asserted on both of its values, so a
//! change to one that silently disagrees with the other fails here.
//!
//! Asserted against `lang.strings().status_self_registered`, the i18n value
//! the badge renders — not the English literal, which a translation change
//! would be free to touch.

use super::*;
use sui_id_shared::ids::ClientId;

const LANG: sui_id_i18n::Locale = sui_id_i18n::Locale::En;

fn marker() -> &'static str {
    LANG.strings().status_self_registered
}

fn client_summary(registered_via: &str) -> ClientSummary {
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
        created_at: chrono::Utc::now(),
    }
}

fn client_edit_data(registered_via: &str) -> ClientEditData {
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
        freshly_rotated_secret: None,
    }
}

#[test]
fn render_clients_marks_a_dynamically_registered_client() {
    let html = render_clients(
        true,
        vec![client_summary("dynamic")],
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
        vec![client_summary("admin")],
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
    let html = render_client_edit(true, client_edit_data("dynamic"), None, "csrf".into(), LANG);
    assert!(
        html.contains(marker()),
        "the edit view must carry the marker for a dynamically registered client"
    );
}

#[test]
fn render_client_edit_does_not_mark_an_admin_created_client() {
    let html = render_client_edit(true, client_edit_data("admin"), None, "csrf".into(), LANG);
    assert!(
        !html.contains(marker()),
        "the edit view must not carry the marker for an administrator-created client"
    );
}
