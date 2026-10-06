//! Properties and units around `lockout_backoff`. The function
//! itself is a small piece of arithmetic, but it sits on the
//! hottest security-decision path in sui-id, so it earns dense
//! testing.

use super::lockout_backoff;
use proptest::prelude::*;

#[tokio::test]
async fn first_two_failures_yield_no_lock() {
    // Operators routinely fat-finger a password; the first two
    // attempts must have no observable consequence beyond
    // bumping the failure counter.
    assert_eq!(lockout_backoff(1, 24 * 60 * 60), None);
    assert_eq!(lockout_backoff(2, 24 * 60 * 60), None);
}

#[tokio::test]
async fn third_failure_yields_a_short_lock() {
    let d = lockout_backoff(3, 24 * 60 * 60).expect("lock at 3rd failure");
    assert_eq!(d.num_seconds(), 30);
}

#[tokio::test]
async fn lock_window_is_capped_at_max_secs() {
    // Ninth+ failure on the curve hits 12h+; with a 1-hour cap
    // we should see exactly 1 hour, never higher.
    let cap = 60 * 60;
    for n in 9..20 {
        let d = lockout_backoff(n, cap).expect("locked");
        assert!(
            d.num_seconds() <= cap,
            "failure {n} produced {} s, exceeds cap {} s",
            d.num_seconds(),
            cap
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        ..ProptestConfig::default()
    })]

    /// The curve must be monotonically non-decreasing: a higher
    /// failure count never produces a *shorter* lock than a
    /// lower one (within the same cap). A regression that
    /// flipped the table around would let an attacker
    /// preferentially time more attempts.
    #[test]
    fn backoff_is_monotone_in_failure_count(
        cap in 1i64..(48 * 60 * 60),
        a in 1i64..15,
        b in 1i64..15,
    ) {
        prop_assume!(a <= b);
        let da = lockout_backoff(a, cap).map(|d| d.num_seconds()).unwrap_or(0);
        let db = lockout_backoff(b, cap).map(|d| d.num_seconds()).unwrap_or(0);
        prop_assert!(db >= da, "{a} -> {da}s, {b} -> {db}s");
    }

    /// No matter how many failures or how the curve evolves, the
    /// returned window never exceeds the operator-set cap. This
    /// is the property the configuration knob is supposed to
    /// give us — operators choose 15min and they get 15min.
    #[test]
    fn backoff_is_bounded_by_max_secs(
        cap in 1i64..(48 * 60 * 60),
        n in 1i64..50,
    ) {
        if let Some(d) = lockout_backoff(n, cap) {
            prop_assert!(d.num_seconds() <= cap);
        }
    }
}
