use super::*;
use crate::security::SecurityLevel;

#[test]
fn hash_then_verify_roundtrips() {
    let pw = "correct horse battery staple";
    let phc = hash_password(pw).expect("hash");
    verify_password(pw, &phc).expect("verify");
}

#[test]
fn wrong_password_is_rejected() {
    let phc = hash_password("a-very-strong-password").expect("hash");
    let r = verify_password("not the right password", &phc);
    assert!(matches!(r, Err(CoreError::InvalidCredentials)));
}

#[test]
fn malformed_stored_hash_returns_password_error() {
    let r = verify_password("anything", "this is not phc");
    assert!(matches!(r, Err(CoreError::Password)));
}

#[test]
fn policy_rejects_short_passwords_at_standard_level() {
    let min = SecurityLevel::Standard.password_min_len();
    let r = check_password_policy("short", min);
    assert!(matches!(r, Err(CoreError::BadRequest(_))));
}

#[test]
fn policy_accepts_standard_minimum_length() {
    let min = SecurityLevel::Standard.password_min_len();
    // "a-perfectly-" = 12 chars — exactly at the Standard floor
    check_password_policy("a-perfectly-", min).expect("policy");
}

#[test]
fn policy_accepts_reasonable_length_password() {
    let min = SecurityLevel::Standard.password_min_len();
    check_password_policy("a-perfectly-reasonable-pass", min).expect("policy");
}

/// In Development mode, 8-char passwords (e.g. "changeme") must be accepted.
/// This ensures the dev-mode relaxation works correctly without touching
/// the Standard-level path.
#[test]
fn policy_accepts_dev_password_at_development_level() {
    let min = SecurityLevel::Development.password_min_len();
    check_password_policy("changeme", min).expect("dev-mode 8-char password");
}

/// Passwords shorter than the Development floor are still rejected.
#[test]
fn policy_rejects_too_short_even_at_development_level() {
    let min = SecurityLevel::Development.password_min_len();
    let r = check_password_policy("short", min);
    assert!(matches!(r, Err(CoreError::BadRequest(_))));
}

/// Standard-level must reject a Development-only password.
#[test]
fn standard_rejects_development_password() {
    let min = SecurityLevel::Standard.password_min_len();
    let r = check_password_policy("changeme", min); // 8 chars < 12
    assert!(matches!(r, Err(CoreError::BadRequest(_))));
}

// ---------- property-based tests (v0.13.0) ----------
//
// Two invariants for the password-hash path:
//
//   1. verify_password(p, hash(p)) succeeds.
//   2. verify_password(other, hash(p)) fails.
//
// Argon2id is intentionally slow (production parameters target tens
// of ms per call), so we cap proptest cases tight — under 30 — to
// keep `cargo test` from blowing past a reasonable budget.

use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig {
        // Argon2id at production parameters is ~80 ms per hash. Each
        // property does 1-2 hashes per case. We cap aggressively at
        // 4 cases per property — enough to demonstrate the property,
        // tight enough to keep `cargo test --lib` snappy. Operators
        // who want broader coverage can override at runtime via
        // `PROPTEST_CASES=128 cargo test`.
        cases: 4,
        ..ProptestConfig::default()
    })]

    #[test]
    fn verify_succeeds_for_any_round_trip(
        // ASCII-only; Argon2 itself accepts arbitrary bytes but the
        // sui-id setup endpoint won't, so the realistic input space
        // is the printable ASCII range.
        password in "[ -~]{12,64}",
    ) {
        let hash = hash_password(&password).expect("hash");
        prop_assert!(verify_password(&password, &hash).is_ok());
    }

    #[test]
    fn verify_fails_on_any_distinct_password(
        password in "[ -~]{12,64}",
        other in "[ -~]{12,64}",
    ) {
        prop_assume!(password != other);
        let hash = hash_password(&password).expect("hash");
        prop_assert!(verify_password(&other, &hash).is_err());
    }

    #[test]
    fn hashes_differ_across_invocations_for_same_password(
        password in "[ -~]{12,64}",
    ) {
        // Argon2id with a random salt should produce a distinct
        // hash every time. If this were not true, two users
        // with the same password would share a hash and the
        // database would leak that fact. This property guards
        // against an accidental zero-salt regression.
        let h1 = hash_password(&password).expect("hash 1");
        let h2 = hash_password(&password).expect("hash 2");
        prop_assert_ne!(h1, h2);
    }
}
