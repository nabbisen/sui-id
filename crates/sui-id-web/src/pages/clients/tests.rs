//! RFC 136 D3 (step 2): the test that holds the self-registered marker.
//! RFC 095 M3 stage 4a §5 extends this file with the auth-method-unrecorded
//! marker, on the same two render sites, same shape. RFC 149 Q4 rebuilds
//! the create/edit forms around the shared `AppFormData`/`render_app_form`
//! — the marker tests below are otherwise unchanged.
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

/// A default `AppFormData` with one empty redirect URI row — the shape
/// every fresh render starts from, create or edit.
fn default_form(action_url: &str, submit_label: &'static str, cancel_url: &str) -> AppFormData {
    AppFormData {
        action_url: action_url.into(),
        name: "a client".into(),
        name_error: None,
        redirect_uris: vec!["https://example.com/cb".into()],
        redirect_uri_errors: vec![None],
        post_logout_uris: vec![],
        post_logout_uri_errors: vec![],
        standard_scopes: vec![
            ("profile", true),
            ("email", true),
            ("offline_access", false),
        ],
        scopes_notice: None,
        additional_scopes: String::new(),
        additional_scopes_error: None,
        consent_policy: "first_time".into(),
        homepage_uri: String::new(),
        homepage_uri_error: None,
        privacy_policy_uri: String::new(),
        privacy_policy_uri_error: None,
        tos_uri: String::new(),
        tos_uri_error: None,
        confidential: Some(true),
        csrf_token: "csrf".into(),
        submit_label,
        cancel_url: cancel_url.into(),
    }
}

fn client_edit_data(registered_via: &str, auth_method: Option<&str>) -> ClientEditData {
    let now = chrono::Utc::now();
    let mut form = default_form("/admin/clients/x/edit", "Save", "/admin/clients");
    form.confidential = None; // edit: immutable, shown read-only elsewhere
    ClientEditData {
        id: "11111111-1111-1111-1111-111111111111".into(),
        confidential: true,
        is_disabled: false,
        created_at: now,
        updated_at: now,
        registered_via: registered_via.into(),
        token_endpoint_auth_method: auth_method.map(str::to_string),
        auth_method_unrecognized: false,
        freshly_rotated_secret: None,
        username: "admin".into(),
        can_write: true,
        form,
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
        "admin".into(),
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
        "admin".into(),
    );
    assert!(
        !html.contains(marker()),
        "an administrator-created client must not carry the self-registered marker"
    );
}

#[test]
fn render_client_edit_marks_a_dynamically_registered_client() {
    let html = render_client_edit(
        client_edit_data("dynamic", Some("client_secret_basic")),
        None,
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
        client_edit_data("admin", Some("client_secret_basic")),
        None,
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
        "admin".into(),
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
        "admin".into(),
    );
    assert!(
        !html.contains(unrecorded_marker()),
        "a client with a recorded token_endpoint_auth_method must not be \
         rendered as unrecorded"
    );
}

#[test]
fn render_client_edit_marks_a_client_with_no_recorded_auth_method() {
    let html = render_client_edit(client_edit_data("admin", None), None, LANG);
    assert!(
        html.contains(unrecorded_marker()),
        "the edit view must carry the unrecorded marker when the auth method \
         was never recorded"
    );
}

#[test]
fn render_client_edit_does_not_mark_a_client_with_a_recorded_auth_method() {
    let html = render_client_edit(
        client_edit_data("admin", Some("client_secret_basic")),
        None,
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
    let html = render_clients(
        true,
        vec![summary],
        None,
        None,
        "csrf".into(),
        false,
        LANG,
        "admin".into(),
    );
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
    let html = render_client_edit(data, None, LANG);
    assert!(
        html.contains(unrecognized_marker()),
        "the edit view must carry the unrecognized marker when the stored \
         value is unparseable"
    );
    assert!(!html.contains(unrecorded_marker()));
}

// ── RFC 149 Q4 §3/§5 test 8: create and edit share one component ──────────

fn form_field_names(html: &str) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    let mut search_from = 0usize;
    while let Some(rel) = html[search_from..].find("name=\"") {
        let start = search_from + rel + "name=\"".len();
        let end = start + html[start..].find('"').expect("closing quote");
        out.insert(html[start..end].to_string());
        search_from = end;
    }
    out.remove("_csrf");
    out
}

/// RFC 149 Q4 §3/§5 test 8: the create and edit forms offer the same field
/// set, except the few that exist only on one side, each for a named
/// reason (not an accident this RFC is meant to close):
/// - `confidential` — create only; immutable once set (edit's own "basic
///   info" card shows it read-only instead).
#[test]
fn create_and_edit_offer_the_same_fields_except_the_named_exceptions() {
    let create_html = render_clients_new(
        default_form("/admin/clients", "Register", "/admin/clients"),
        None,
        false,
        LANG,
        "admin".into(),
    );
    let edit_html = render_client_edit(client_edit_data("admin", None), None, LANG);

    let create_fields = form_field_names(&create_html);
    let edit_fields = form_field_names(&edit_html);

    let create_only = ["confidential"];

    let missing_from_edit: Vec<&String> = create_fields
        .iter()
        .filter(|f| !edit_fields.contains(f.as_str()) && !create_only.contains(&f.as_str()))
        .collect();
    assert!(
        missing_from_edit.is_empty(),
        "the edit form is missing field(s) the create form has: {missing_from_edit:?}"
    );

    let missing_from_create: Vec<&String> = edit_fields
        .iter()
        .filter(|f| !create_fields.contains(f.as_str()))
        .collect();
    assert!(
        missing_from_create.is_empty(),
        "the create form is missing field(s) the edit form has: {missing_from_create:?}"
    );

    assert!(
        create_fields.contains("confidential") && !edit_fields.contains("confidential"),
        "confidential must be create-only"
    );
}

/// The three application-identity URL fields use `type="url"`, which gives
/// a phone's URL keyboard and the browser's own check — the server's own
/// validation is unchanged and remains the authority.
#[test]
fn url_fields_use_the_url_input_type() {
    let html = render_client_edit(client_edit_data("admin", None), None, LANG);
    for field in ["homepage_uri", "privacy_policy_uri", "tos_uri"] {
        assert!(
            html.contains(&format!("name=\"{field}\" type=\"url\"")),
            "{field} must be type=\"url\": {html}"
        );
    }
}

/// Q4's review: with JavaScript off, *Add another* and *Remove* are
/// submit buttons, and the browser's own validation blocked them while
/// any required field (the app's name) was empty, showing a "fill in
/// this field" bubble instead of a new row. The tests that post the form
/// directly could not see it; a browser walk did. Every submit button
/// that only changes the rows must skip that validation; *Save* must not.
#[test]
fn the_row_buttons_skip_browser_validation_and_save_does_not() {
    let mut data = client_edit_data("admin", Some("client_secret_basic"));
    data.form.redirect_uris = vec!["https://a.example/cb".into(), "https://b.example/cb".into()];
    data.form.redirect_uri_errors = vec![None, None];
    data.form.post_logout_uris = vec!["https://a.example/out".into()];
    data.form.post_logout_uri_errors = vec![None];
    let html = render_client_edit(data, None, LANG);
    let mut row_buttons = 0;
    let mut from = 0;
    while let Some(rel) = html[from..].find("<button") {
        let start = from + rel;
        let end = start + html[start..].find('>').expect("tag ends");
        let tag = &html[start..end];
        from = end;
        let is_row_button = [
            "add_redirect_uri",
            "add_post_logout_uri",
            "remove_redirect_uri",
            "remove_post_logout_uri",
        ]
        .iter()
        .any(|n| tag.contains(&format!("name=\"{n}\"")));
        if is_row_button {
            row_buttons += 1;
            assert!(
                tag.contains("formnovalidate"),
                "a row button must skip validation: {tag}"
            );
        }
        if tag.contains("name=\"save\"") {
            assert!(
                !tag.contains("formnovalidate"),
                "Save must keep the browser's validation: {tag}"
            );
        }
    }
    // Two add buttons, two remove buttons for redirect URIs, one for post-logout.
    assert_eq!(row_buttons, 5, "every row button was checked");
}
