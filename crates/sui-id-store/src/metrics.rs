//! Application metrics registry (RFC 006).
//!
//! A single [`Metrics`] value holds every Prometheus counter, gauge, and
//! histogram for the process.  It is constructed once at startup, wrapped
//! in `Arc<Metrics>`, and stored on [`crate::AppState`] (binary crate).
//! Call sites call the typed increment helpers; they never touch the
//! Prometheus library directly.
//!
//! # Catalog
//!
//! The published metric catalog is defined by this module and is the
//! **only** permissible set.  Additions require an RFC-level review (P3/P4).
//! No per-user, per-client, or per-IP labels ever appear here.
//!
//! ## Counters
//!
//! | Metric | Labels |
//! |---|---|
//! | `sui_id_signin_attempts_total` | `result` |
//! | `sui_id_signin_via_passkey_total` | — |
//! | `sui_id_token_issued_total` | `kind` |
//! | `sui_id_token_revoked_total` | `reason` |
//! | `sui_id_mfa_enrolled_total` | `kind` |
//! | `sui_id_mfa_recovery_consumed_total` | — |
//! | `sui_id_forgot_password_requested_total` | — |
//! | `sui_id_audit_appended_total` | — |
//! | `sui_id_email_outbox_enqueued_total` | — |
//! | `sui_id_email_outbox_failed_total` | `reason` |
//!
//! ## Gauges
//!
//! | Metric |
//! |---|
//! | `sui_id_active_sessions` |
//! | `sui_id_signing_keys_active` |
//! | `sui_id_signing_keys_retired` |
//!
//! ## Histograms
//!
//! | Metric | Labels | Buckets |
//! |---|---|---|
//! | `sui_id_http_request_duration_seconds` | `route`, `status_class` | 5ms…10s |
//! | `sui_id_argon2_verify_duration_seconds` | — | 10ms…5s |

use prometheus::{
    Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, Opts, Registry,
};
use std::sync::Arc;

// ── Label value constants — the published label vocabulary ────────────────────

/// Label values for `sui_id_signin_attempts_total{result=…}`.
pub mod signin_result {
    pub const SUCCESS: &str = "success";
    pub const WRONG_PASSWORD: &str = "wrong_password";
    pub const LOCKED: &str = "locked";
    pub const MFA_FAILED: &str = "mfa_failed";
    pub const DISABLED: &str = "disabled";
}

/// Label values for `sui_id_token_issued_total{kind=…}`.
pub mod token_kind {
    pub const ACCESS: &str = "access";
    pub const REFRESH: &str = "refresh";
    pub const ID: &str = "id";
}

/// Label values for `sui_id_token_revoked_total{reason=…}`.
pub mod revoke_reason {
    pub const LOGOUT: &str = "logout";
    pub const ADMIN: &str = "admin";
    pub const THEFT_DETECTED: &str = "theft_detected";
    pub const EXPIRED_GC: &str = "expired_gc";
}

/// Label values for `sui_id_mfa_enrolled_total{kind=…}`.
pub mod mfa_kind {
    pub const TOTP: &str = "totp";
    pub const WEBAUTHN: &str = "webauthn";
}

/// Label values for `sui_id_email_outbox_failed_total{reason=…}`.
pub mod outbox_fail_reason {
    pub const TRANSPORT: &str = "transport";
    pub const TEMPLATE: &str = "template";
    pub const PERMANENT: &str = "permanent";
}

// ── Metrics registry ─────────────────────────────────────────────────────────

/// All application metrics for a sui-id process.
///
/// Constructed once at startup via [`Metrics::new`], then cloned cheaply
/// via [`Arc<Metrics>`].  Individual call sites call the typed increment
/// helpers; the [`Registry`] is exposed only for the `/metrics` endpoint
/// handler.
pub struct Metrics {
    /// Prometheus registry that owns all of the metrics below.
    pub registry: Registry,

    // ── Counters ─────────────────────────────────────────────────────────
    pub signin_attempts_total: IntCounterVec,
    pub signin_via_passkey_total: IntCounter,
    pub token_issued_total: IntCounterVec,
    pub token_revoked_total: IntCounterVec,
    pub mfa_enrolled_total: IntCounterVec,
    pub mfa_recovery_consumed_total: IntCounter,
    pub forgot_password_requested_total: IntCounter,
    pub audit_appended_total: IntCounter,
    pub email_outbox_enqueued_total: IntCounter,
    pub email_outbox_failed_total: IntCounterVec,

    // ── Gauges ───────────────────────────────────────────────────────────
    pub active_sessions: IntGauge,
    pub signing_keys_active: IntGauge,
    pub signing_keys_retired: IntGauge,

    // ── Histograms ───────────────────────────────────────────────────────
    pub http_request_duration_seconds: HistogramVec,
    pub argon2_verify_duration_seconds: Histogram,
}

impl Metrics {
    /// Construct and register all metrics.
    ///
    /// Returns `Err` only if the metric definitions themselves are invalid
    /// (names / label names break Prometheus naming rules).  In practice
    /// this can only happen during development if a metric is renamed to
    /// an invalid identifier.
    pub fn new() -> Result<Arc<Self>, prometheus::Error> {
        let registry = Registry::new();

        // ── Counters ─────────────────────────────────────────────────────

        let signin_attempts_total = IntCounterVec::new(
            Opts::new(
                "sui_id_signin_attempts_total",
                "Sign-in attempts by result.",
            ),
            &["result"],
        )?;
        registry.register(Box::new(signin_attempts_total.clone()))?;

        let signin_via_passkey_total = IntCounter::with_opts(Opts::new(
            "sui_id_signin_via_passkey_total",
            "Successful passkey sign-ins.",
        ))?;
        registry.register(Box::new(signin_via_passkey_total.clone()))?;

        let token_issued_total = IntCounterVec::new(
            Opts::new("sui_id_token_issued_total", "Tokens issued by kind."),
            &["kind"],
        )?;
        registry.register(Box::new(token_issued_total.clone()))?;

        let token_revoked_total = IntCounterVec::new(
            Opts::new("sui_id_token_revoked_total", "Tokens revoked by reason."),
            &["reason"],
        )?;
        registry.register(Box::new(token_revoked_total.clone()))?;

        let mfa_enrolled_total = IntCounterVec::new(
            Opts::new("sui_id_mfa_enrolled_total", "MFA factors enrolled by kind."),
            &["kind"],
        )?;
        registry.register(Box::new(mfa_enrolled_total.clone()))?;

        let mfa_recovery_consumed_total = IntCounter::with_opts(Opts::new(
            "sui_id_mfa_recovery_consumed_total",
            "Recovery codes consumed.",
        ))?;
        registry.register(Box::new(mfa_recovery_consumed_total.clone()))?;

        let forgot_password_requested_total = IntCounter::with_opts(Opts::new(
            "sui_id_forgot_password_requested_total",
            "Forgot-password requests submitted.",
        ))?;
        registry.register(Box::new(forgot_password_requested_total.clone()))?;

        let audit_appended_total = IntCounter::with_opts(Opts::new(
            "sui_id_audit_appended_total",
            "Audit log rows appended.",
        ))?;
        registry.register(Box::new(audit_appended_total.clone()))?;

        let email_outbox_enqueued_total = IntCounter::with_opts(Opts::new(
            "sui_id_email_outbox_enqueued_total",
            "Emails enqueued in the outbox.",
        ))?;
        registry.register(Box::new(email_outbox_enqueued_total.clone()))?;

        let email_outbox_failed_total = IntCounterVec::new(
            Opts::new(
                "sui_id_email_outbox_failed_total",
                "Outbox delivery failures by reason.",
            ),
            &["reason"],
        )?;
        registry.register(Box::new(email_outbox_failed_total.clone()))?;

        // ── Gauges ───────────────────────────────────────────────────────

        let active_sessions = IntGauge::with_opts(Opts::new(
            "sui_id_active_sessions",
            "Sessions that are neither revoked nor idle-timed-out.",
        ))?;
        registry.register(Box::new(active_sessions.clone()))?;

        let signing_keys_active = IntGauge::with_opts(Opts::new(
            "sui_id_signing_keys_active",
            "Active (in-use) signing keys.",
        ))?;
        registry.register(Box::new(signing_keys_active.clone()))?;

        let signing_keys_retired = IntGauge::with_opts(Opts::new(
            "sui_id_signing_keys_retired",
            "Retired (revoked) signing keys.",
        ))?;
        registry.register(Box::new(signing_keys_retired.clone()))?;

        // ── Histograms ───────────────────────────────────────────────────

        // HTTP latency buckets: 5ms, 25ms, 100ms, 500ms, 2s, 10s
        let http_buckets = vec![0.005, 0.025, 0.1, 0.5, 2.0, 10.0];
        let http_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "sui_id_http_request_duration_seconds",
                "HTTP request latency by route template and response status class.",
            )
            .buckets(http_buckets),
            &["route", "status_class"],
        )?;
        registry.register(Box::new(http_request_duration_seconds.clone()))?;

        // Argon2 buckets: 10ms, 50ms, 100ms, 250ms, 500ms, 1s, 2s, 5s
        let argon2_buckets = vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0];
        let argon2_verify_duration_seconds = Histogram::with_opts(
            HistogramOpts::new(
                "sui_id_argon2_verify_duration_seconds",
                "Time spent in Argon2id password verification (including the \
                 constant-time dummy path). Useful for detecting parameter drift.",
            )
            .buckets(argon2_buckets),
        )?;
        registry.register(Box::new(argon2_verify_duration_seconds.clone()))?;

        Ok(Arc::new(Self {
            registry,
            signin_attempts_total,
            signin_via_passkey_total,
            token_issued_total,
            token_revoked_total,
            mfa_enrolled_total,
            mfa_recovery_consumed_total,
            forgot_password_requested_total,
            audit_appended_total,
            email_outbox_enqueued_total,
            email_outbox_failed_total,
            active_sessions,
            signing_keys_active,
            signing_keys_retired,
            http_request_duration_seconds,
            argon2_verify_duration_seconds,
        }))
    }

    // ── Typed increment helpers ───────────────────────────────────────────────
    // These are the only call-site API; call sites never import prometheus types.

    /// Record a sign-in attempt with the given result label.
    /// Use the constants in [`signin_result`].
    pub fn signin(&self, result: &str) {
        self.signin_attempts_total
            .with_label_values(&[result])
            .inc();
    }

    /// Record a successful passkey sign-in.
    pub fn signin_passkey(&self) {
        self.signin_via_passkey_total.inc();
    }

    /// Record a token issuance.  Use the constants in [`token_kind`].
    pub fn token_issued(&self, kind: &str) {
        self.token_issued_total.with_label_values(&[kind]).inc();
    }

    /// Record a token revocation.  Use the constants in [`revoke_reason`].
    pub fn token_revoked(&self, reason: &str) {
        self.token_revoked_total.with_label_values(&[reason]).inc();
    }

    /// Record an MFA factor enrolment.  Use the constants in [`mfa_kind`].
    pub fn mfa_enrolled(&self, kind: &str) {
        self.mfa_enrolled_total.with_label_values(&[kind]).inc();
    }

    /// Record a recovery-code consumption.
    pub fn mfa_recovery_consumed(&self) {
        self.mfa_recovery_consumed_total.inc();
    }

    /// Record a forgot-password request.
    pub fn forgot_password_requested(&self) {
        self.forgot_password_requested_total.inc();
    }

    /// Record an audit log row being appended.
    pub fn audit_appended(&self) {
        self.audit_appended_total.inc();
    }

    /// Record an email being enqueued in the outbox.
    pub fn email_outbox_enqueued(&self) {
        self.email_outbox_enqueued_total.inc();
    }

    /// Record an outbox delivery failure.  Use the constants in
    /// [`outbox_fail_reason`].
    pub fn email_outbox_failed(&self, reason: &str) {
        self.email_outbox_failed_total
            .with_label_values(&[reason])
            .inc();
    }

    /// Set the active-sessions gauge (call after login/logout/session-purge).
    pub fn set_active_sessions(&self, n: i64) {
        self.active_sessions.set(n);
    }

    /// Set the signing-key gauge values.
    pub fn set_signing_keys(&self, active: i64, retired: i64) {
        self.signing_keys_active.set(active);
        self.signing_keys_retired.set(retired);
    }

    /// Observe an HTTP request duration.
    ///
    /// `route` must be a fixed template (e.g. `"/admin/users/{id}"`) — never a
    /// raw URL — to bound cardinality (P4).  `status_class` is one of
    /// `"2xx"`, `"3xx"`, `"4xx"`, `"5xx"`.
    pub fn observe_http(&self, route: &str, status_class: &str, duration_secs: f64) {
        self.http_request_duration_seconds
            .with_label_values(&[route, status_class])
            .observe(duration_secs);
    }

    /// Observe an Argon2id verification duration (including the dummy path).
    pub fn observe_argon2(&self, duration_secs: f64) {
        self.argon2_verify_duration_seconds.observe(duration_secs);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
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
        let names: Vec<_> = families.iter().map(|f| f.get_name().to_owned()).collect();

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
                !name.contains("user_id")
                    && !name.contains("client_id")
                    && !name.contains("ip_addr"),
                "metric {name} looks like it contains a PII label — violates P3/P4"
            );
        }
    }
}
