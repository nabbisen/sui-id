use super::*;

#[test]
fn metrics_registry_constructs_without_error() {
    Metrics::new().expect("Metrics::new must succeed with valid metric definitions");
}

#[test]
fn signin_counter_increments() {
    let m = Metrics::new().unwrap();
    m.signin(signin_result::WRONG_PASSWORD);
    m.signin(signin_result::WRONG_PASSWORD);
    m.signin(signin_result::SUCCESS);
    let count = m
        .signin_attempts_total
        .with_label_values(&[signin_result::WRONG_PASSWORD])
        .get();
    assert_eq!(count, 2, "wrong_password counter must be 2");
    let ok_count = m
        .signin_attempts_total
        .with_label_values(&[signin_result::SUCCESS])
        .get();
    assert_eq!(ok_count, 1, "success counter must be 1");
}

#[test]
fn token_counters_increment_by_kind() {
    let m = Metrics::new().unwrap();
    m.token_issued(token_kind::ACCESS);
    m.token_issued(token_kind::ACCESS);
    m.token_issued(token_kind::REFRESH);
    assert_eq!(
        m.token_issued_total
            .with_label_values(&[token_kind::ACCESS])
            .get(),
        2
    );
    assert_eq!(
        m.token_issued_total
            .with_label_values(&[token_kind::REFRESH])
            .get(),
        1
    );
}

#[test]
fn gauges_set_correctly() {
    let m = Metrics::new().unwrap();
    m.set_active_sessions(42);
    assert_eq!(m.active_sessions.get(), 42);
    m.set_signing_keys(1, 3);
    assert_eq!(m.signing_keys_active.get(), 1);
    assert_eq!(m.signing_keys_retired.get(), 3);
}

/// Verifies that (a) all expected metric families appear in the registry
/// after being observed, and (b) no metric name contains PII-like
/// substrings (P3/P4).
///
/// `registry.gather()` only returns metric families that have been
/// observed at least once, so we pre-seed each labelled counter before
/// gathering.
#[test]
fn published_catalog_label_set_is_bounded() {
    let m = Metrics::new().unwrap();

    // Pre-seed all labelled series so they appear in gather() output.
    m.signin(signin_result::SUCCESS);
    m.signin(signin_result::WRONG_PASSWORD);
    m.signin(signin_result::LOCKED);
    m.signin(signin_result::MFA_FAILED);
    m.signin(signin_result::DISABLED);
    m.signin_passkey();
    m.token_issued(token_kind::ACCESS);
    m.token_issued(token_kind::REFRESH);
    m.token_issued(token_kind::ID);
    m.token_revoked(revoke_reason::LOGOUT);
    m.token_revoked(revoke_reason::ADMIN);
    m.token_revoked(revoke_reason::THEFT_DETECTED);
    m.token_revoked(revoke_reason::EXPIRED_GC);
    m.mfa_enrolled(mfa_kind::TOTP);
    m.mfa_enrolled(mfa_kind::WEBAUTHN);
    m.mfa_recovery_consumed();
    m.forgot_password_requested();
    m.audit_appended();
    m.email_outbox_enqueued();
    m.email_outbox_failed(outbox_fail_reason::TRANSPORT);
    m.email_outbox_failed(outbox_fail_reason::TEMPLATE);
    m.email_outbox_failed(outbox_fail_reason::PERMANENT);
    m.set_active_sessions(1);
    m.set_signing_keys(1, 0);
    m.observe_http("/admin/users", "2xx", 0.01);
    m.observe_argon2(0.1);

    let families = m.registry.gather();
    let names: Vec<_> = families.iter().map(|f| f.name().to_owned()).collect();

    // Every expected metric must appear in the gathered output.
    let expected = [
        "sui_id_signin_attempts_total",
        "sui_id_signin_via_passkey_total",
        "sui_id_token_issued_total",
        "sui_id_token_revoked_total",
        "sui_id_mfa_enrolled_total",
        "sui_id_mfa_recovery_consumed_total",
        "sui_id_forgot_password_requested_total",
        "sui_id_audit_appended_total",
        "sui_id_email_outbox_enqueued_total",
        "sui_id_email_outbox_failed_total",
        "sui_id_active_sessions",
        "sui_id_signing_keys_active",
        "sui_id_signing_keys_retired",
        "sui_id_http_request_duration_seconds",
        "sui_id_argon2_verify_duration_seconds",
    ];
    for name in &expected {
        let found = names
            .iter()
            .any(|n| n.as_str() == *name || n.starts_with(name));
        assert!(
            found,
            "expected metric {name} missing from registry — catalog drift?"
        );
    }

    // No family name may contain PII-like substrings (P3/P4).
    for name in &names {
        assert!(
            !name.contains("user_id") && !name.contains("client_id") && !name.contains("ip_addr"),
            "metric {name} looks like it contains a PII label — violates P3/P4"
        );
    }
}
