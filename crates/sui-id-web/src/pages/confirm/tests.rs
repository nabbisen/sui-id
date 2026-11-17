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
