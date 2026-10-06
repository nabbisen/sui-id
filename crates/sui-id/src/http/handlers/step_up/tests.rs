use super::sanitise_return_to;

// ── RFC 089: allowlist enforcement ────────────────────────────────────

#[test]
fn allowlisted_admin_users_path_accepted() {
    let result = sanitise_return_to("/admin/users/some-uuid/delete-confirm");
    assert_eq!(result, "/admin/users/some-uuid/delete-confirm");
}

#[test]
fn allowlisted_admin_signing_keys_path_accepted() {
    let result = sanitise_return_to("/admin/signing-keys/some-id/delete-confirm");
    assert_eq!(result, "/admin/signing-keys/some-id/delete-confirm");
}

#[test]
fn allowlisted_admin_settings_path_accepted() {
    let result = sanitise_return_to("/admin/settings/security");
    assert_eq!(result, "/admin/settings/security");
}

#[test]
fn allowlisted_me_security_path_accepted() {
    let result = sanitise_return_to("/me/security/mfa");
    assert_eq!(result, "/me/security/mfa");
}

#[test]
fn non_allowlisted_admin_dashboard_collapses_to_default() {
    let result = sanitise_return_to("/admin/dashboard");
    assert_eq!(
        result, "/me/security",
        "admin dashboard is not step-up gated"
    );
}

#[test]
fn non_allowlisted_root_collapses_to_default() {
    let result = sanitise_return_to("/");
    assert_eq!(result, "/me/security");
}

#[test]
fn non_allowlisted_admin_bare_collapses_to_default() {
    let result = sanitise_return_to("/admin");
    assert_eq!(result, "/me/security");
}

// ── Existing format-check tests (unchanged) ───────────────────────────

#[test]
fn empty_string_collapses() {
    assert_eq!(sanitise_return_to(""), "/me/security");
}

#[test]
fn absolute_url_collapses() {
    assert_eq!(sanitise_return_to("https://evil.example/"), "/me/security");
}

#[test]
fn protocol_relative_collapses() {
    assert_eq!(sanitise_return_to("//evil.example"), "/me/security");
}

#[test]
fn backslash_prefix_collapses() {
    assert_eq!(sanitise_return_to("/\\evil"), "/me/security");
}

#[test]
fn embedded_newline_collapses() {
    assert_eq!(
        sanitise_return_to("/admin/users/\nX-Header: injected"),
        "/me/security"
    );
}

#[test]
fn embedded_nul_collapses() {
    assert_eq!(sanitise_return_to("/admin/users/\0etc"), "/me/security");
}
