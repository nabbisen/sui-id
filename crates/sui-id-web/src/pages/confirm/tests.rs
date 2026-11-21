//! RFC 144 (Refresh, §5 / N2a review F5): the four confirmations with
//! an optional reason all say "Reason (optional)" — not "Reason for
//! disabling (optional)", which was wrong for three of the four
//! (deleting a user, resetting MFA, deleting an app, deleting a
//! signing key: none of them disables anything).

use super::*;

const LANG: sui_id_i18n::Locale = sui_id_i18n::Locale::En;

fn csrf() -> String {
    "csrf".into()
}

#[test]
fn delete_user_confirmation_says_reason_optional_not_disabling() {
    let html = render_confirm_delete_user(
        ConfirmDeleteUserData {
            user_id: "u1".into(),
            username: "alice".into(),
            csrf_token: csrf(),
            admin_username: "admin".into(),
        },
        false,
        LANG,
    );
    assert!(html.contains("Reason (optional)"));
    assert!(!html.contains("disabling"));
}

#[test]
fn reset_mfa_confirmation_says_reason_optional_not_disabling() {
    let html = render_confirm_reset_mfa(
        ConfirmResetMfaData {
            user_id: "u1".into(),
            username: "alice".into(),
            csrf_token: csrf(),
            admin_username: "admin".into(),
        },
        false,
        LANG,
    );
    assert!(html.contains("Reason (optional)"));
    assert!(!html.contains("disabling"));
}

#[test]
fn delete_client_confirmation_says_reason_optional_not_disabling() {
    let html = render_confirm_delete_client(
        ConfirmDeleteClientData {
            client_id: "c1".into(),
            client_name: "a client".into(),
            csrf_token: csrf(),
            admin_username: "admin".into(),
        },
        false,
        LANG,
    );
    assert!(html.contains("Reason (optional)"));
    assert!(!html.contains("disabling"));
}

#[test]
fn delete_signing_key_confirmation_says_reason_optional_not_disabling() {
    let html = render_confirm_delete_signing_key(
        ConfirmDeleteSigningKeyData {
            key_id: "k1".into(),
            algorithm: "Ed25519".into(),
            csrf_token: csrf(),
            admin_username: "admin".into(),
        },
        false,
        LANG,
    );
    assert!(html.contains("Reason (optional)"));
    assert!(!html.contains("disabling"));
}

/// The actual disable confirmation (`is_disabled: false`, so the form
/// renders the "disable" action's own reason field) genuinely disables
/// the user, so it is not expected to avoid the word "disabling"
/// elsewhere on the page (`confirm_disable_title`/`_impact` may use
/// it) — only the reason field's own label is checked, same as the
/// other four.
#[test]
fn disable_user_confirmation_also_says_reason_optional() {
    let html = render_confirm_disable_user(
        ConfirmDisableData {
            user_id: "u1".into(),
            username: "alice".into(),
            is_disabled: false,
            csrf_token: csrf(),
            admin_username: "admin".into(),
        },
        false,
        LANG,
    );
    assert!(html.contains("Reason (optional)"));
}

/// The required-reason path (recovery-link issuance) keeps its own
/// "Reason (required)" label, unaffected by this rename.
#[test]
fn recovery_link_confirmation_keeps_reason_required() {
    let html = render_confirm_recovery_link(
        ConfirmRecoveryLinkData {
            user_id: "u1".into(),
            username: "alice".into(),
            csrf_token: csrf(),
            admin_username: "admin".into(),
        },
        false,
        LANG,
    );
    assert!(html.contains("Reason (required)"));
    assert!(!html.contains("Reason (optional)"));
}

/// RFC 144 (Refresh, §3): the reason textarea carries no placeholder
/// attribute — its guidance is the field's own hint text now.
#[test]
fn reason_field_has_no_placeholder() {
    let html = render_confirm_delete_user(
        ConfirmDeleteUserData {
            user_id: "u1".into(),
            username: "alice".into(),
            csrf_token: csrf(),
            admin_username: "admin".into(),
        },
        false,
        LANG,
    );
    assert!(!html.contains("placeholder="));
}

// ── `0.93.0` O6 §4: the confirm page's own impact is no longer muted ────────

/// No confirm page's impact (the page's lead consequence) is rendered
/// inside `class="muted"` any more — it is full-contrast body text
/// (RFC 144 tokens), visually tied to the action it describes.
#[test]
fn no_confirm_page_impact_is_muted() {
    let html = render_confirm_delete_user(
        ConfirmDeleteUserData {
            user_id: "u1".into(),
            username: "alice".into(),
            csrf_token: csrf(),
            admin_username: "admin".into(),
        },
        false,
        LANG,
    );
    // The CSS for `.confirm-impact`/`.muted` is embedded on every page
    // (`<style>`), so these check the *rendered element* around the
    // impact text itself, not a bare substring of the class names.
    let impact_text = "This user will be permanently removed from the user list. \
                        Their audit history is preserved.";
    assert!(html.contains(&format!("<p class=\"confirm-impact\">{impact_text}</p>")));
    assert!(!html.contains(&format!("<p class=\"muted\">{impact_text}</p>")));
}

fn rotate(active_key_count: usize, lang: sui_id_i18n::Locale) -> String {
    render_confirm_rotate_signing_key(
        ConfirmRotateSigningKeyData {
            csrf_token: csrf(),
            active_key_count,
            admin_username: "admin".into(),
        },
        false,
        lang,
    )
}

/// §4 item 3: the rotation page's count no longer builds English text
/// in Rust (the old `"({} active key{} will become retired)"`) — the
/// number is its own prominent element, and the sentence around it
/// comes from the locale.
#[test]
fn rotate_signing_key_count_is_its_own_element_not_english_built_in_rust() {
    let html = rotate(5, LANG);
    assert!(!html.contains("active keys will become retired)"));
    assert!(html.contains("<span class=\"confirm-impact__count\">5</span>"));
    assert!(html.contains("active keys will become retired."));
}

/// §4 item 2: zero gets its own wording, not "0 active keys ...".
#[test]
fn rotate_signing_key_zero_active_keys_gets_its_own_wording() {
    let html = rotate(0, LANG);
    assert!(!html.contains("0 active key"));
    assert!(html.contains("No active keys will become retired."));
    assert!(!html.contains("<span class=\"confirm-impact__count\">"));
}

/// The rotation page renders no English leftovers in `ja` or
/// `zh-Hans` — the count sentence comes entirely from the locale now,
/// not a Rust format string.
#[test]
fn rotate_signing_key_count_has_no_english_in_other_locales() {
    for lang in [sui_id_i18n::Locale::Ja, sui_id_i18n::Locale::ZhHans] {
        let html = rotate(3, lang);
        assert!(
            !html.contains("active key") && !html.contains("will become retired"),
            "{lang:?}: {html}"
        );
        let zero_html = rotate(0, lang);
        assert!(
            !zero_html.contains("active key") && !zero_html.contains("will become retired"),
            "{lang:?} (zero): {zero_html}"
        );
    }
}
