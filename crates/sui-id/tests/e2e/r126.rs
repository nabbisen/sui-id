//! RFC 126 — password hashing must not block the request runtime.
//!
//! D3: a real verification and a `DUMMY_PHC` one pay the same thread-hop
//! cost, so the timing-equalisation RFC 123 built survives moving the hash
//! off the worker thread. D5: the closure requirement — an unrelated,
//! health-check-shaped request is served promptly during a burst of
//! concurrent, *real* (non-dummy) authenticating requests, including the
//! costliest call site in the codebase (`match_recovery_code`'s
//! up-to-eight-hash loop). Every concurrency test here runs under
//! `#[tokio::test(flavor = "multi_thread", worker_threads = 1)]` so the
//! phenomenon is deterministic rather than dependent on how many cores the
//! machine running the suite happens to have.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use sui_id::build_router;
use tower::ServiceExt;

use super::common::*;

async fn get_healthz(state: &sui_id::AppState) -> StatusCode {
    build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/healthz")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("healthz request")
        .status()
}

/// Measure this run's own Argon2 cost once, so a probe deadline can scale
/// with however fast or slow *this* machine or CI runner is, rather than a
/// constant picked on one developer's laptop. This is the fix for exactly
/// what reverted the first attempt at this test (`ff19f05` / `7813b35`): a
/// fixed 30ms deadline cleared by 52ms of ordinary CI scheduling jitter, on
/// a run whose actual hash cost was never measured and could have been
/// anywhere from ~34ms (RFC 123's release-profile measurement) to several
/// hundred milliseconds (debug profile, a slower or more loaded runner).
async fn calibrate_single_hash_cost() -> std::time::Duration {
    use sui_id_core::password::{DUMMY_PHC, verify_password};
    let t0 = std::time::Instant::now();
    let _ = verify_password("calibration-guess-not-a-real-password", DUMMY_PHC).await;
    t0.elapsed()
}

/// A probe deadline derived from this run's own measured hash cost, not
/// picked. A probe genuinely queued behind Argon2 on the one blocked worker
/// waits on the order of one hash per caller ahead of it in the queue — RFC
/// 126's own numbers are ~34ms each in release, up to ~270ms for the
/// costliest call site — so **twice** one hash's cost is already a
/// conservative floor for "this is blocking, not jitter", with a 150ms
/// absolute floor so the deadline stays meaningful on a run whose measured
/// cost happens to be very small. Because the derivation scales with the
/// measurement, a slower runner widens the deadline along with it: there is
/// no fixed environment speed at which this test is expected to become
/// flaky, short of this project's Argon2 parameters themselves changing —
/// and if that turns out to be false in practice, the honest fallback this
/// RFC's own handoff names is to assert the *relationship* (in-burst vs.
/// out-of-burst, measured in the same run) rather than any absolute, which
/// this derivation does not yet do but could be extended to.
fn probe_deadline(single_hash_cost: std::time::Duration) -> std::time::Duration {
    (single_hash_cost * 2).max(std::time::Duration::from_millis(150))
}

async fn post_wrong_password_login(state: &sui_id::AppState) {
    let _ = build_router(state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/login")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "username={USERNAME}&password={}",
                    urlencode("definitely-the-wrong-password")
                )))
                .expect("req"),
        )
        .await;
}

// ---------- D3 ----------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn d3_dummy_and_real_verification_pay_the_same_thread_hop_cost() {
    use std::time::Instant;
    use sui_id_core::password::{DUMMY_PHC, hash_password, verify_password};

    let real_hash = hash_password("a-real-stored-password-for-this-test")
        .await
        .expect("hash");

    const N: usize = 30;
    let mut real_samples = Vec::with_capacity(N);
    let mut dummy_samples = Vec::with_capacity(N);
    for _ in 0..N {
        let t0 = Instant::now();
        let _ = verify_password("a-wrong-guess-of-similar-length", &real_hash).await;
        real_samples.push(t0.elapsed());

        let t0 = Instant::now();
        let _ = verify_password("a-wrong-guess-of-similar-length", DUMMY_PHC).await;
        dummy_samples.push(t0.elapsed());
    }
    real_samples.sort();
    dummy_samples.sort();
    let real_median = real_samples[N / 2];
    let dummy_median = dummy_samples[N / 2];

    // Same order of magnitude, not equality — a thread hop adds its own
    // small, real cost, and asserting it away would be asserting the wrong
    // thing (and would be flaky). A factor of 3 is generous enough to
    // absorb scheduler jitter while still failing hard on the actual
    // regression this guards against: one branch silently skipping the
    // spawn_blocking hop (or the hash itself) would show a gap of two to
    // three orders of magnitude, not a factor of 3.
    let ratio = if real_median > dummy_median {
        real_median.as_secs_f64() / dummy_median.as_secs_f64()
    } else {
        dummy_median.as_secs_f64() / real_median.as_secs_f64()
    };
    assert!(
        ratio < 3.0,
        "real median {real_median:?} vs dummy median {dummy_median:?}: \
         the thread-hop cost is not landing on both branches equally (ratio {ratio:.2})"
    );
}

// ---------- D4 ----------
//
// D4's bound is checked in `sui-id-core`'s own unit tests
// (`authn::password::tests::the_semaphore_never_grants_more_than_its_derived_bound_at_once`),
// not here. A wall-clock timing test of the live semaphore from this side
// cannot tell "the semaphore is missing" from "Argon2's own memory-bandwidth
// contention slowed many concurrent hashes down anyway" — both produce the
// same slower-than-baseline signal on real hardware, which a first attempt
// at this test confirmed directly: removing the semaphore entirely did not
// make it fail. The white-box test asks the semaphore itself instead.

// ---------- D5 ----------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn d5_a_health_check_is_served_promptly_during_a_burst_of_real_login_attempts() {
    let state = test_app();
    let _ = complete_setup_and_login(&state).await;
    let per_probe_deadline = probe_deadline(calibrate_single_hash_cost().await);

    const BURST: usize = 8;
    let mut handles = Vec::with_capacity(BURST);
    for _ in 0..BURST {
        let state = state.clone();
        handles.push(tokio::spawn(async move {
            post_wrong_password_login(&state).await
        }));
    }
    // Probe repeatedly until the whole burst has finished, measuring **wall
    // clock** around the whole iteration (a short yield, then the probe),
    // not `tokio::time::timeout` around the probe alone — a timeout only
    // bounds time after it is first polled, so it cannot see a delay in
    // this task getting a turn at all, which is exactly what a blocked
    // single worker thread causes. Two failed designs got here: probing in
    // a tight loop with no yield starves the spawned burst entirely (Tokio
    // schedules a task that keeps re-waking itself, via its LIFO slot,
    // ahead of ones sitting in the general run queue); yielding via
    // `sleep` but starting the clock *after* the sleep hides the stall
    // inside the part that isn't measured. Starting the clock before the
    // yield and reading it after the probe catches it either way.
    let mut probes = 0usize;
    while handles.iter().any(|h| !h.is_finished()) {
        let t0 = std::time::Instant::now();
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let status = get_healthz(&state).await;
        let elapsed = t0.elapsed();
        assert!(
            elapsed < per_probe_deadline,
            "a probe iteration took {elapsed:?} (deadline {per_probe_deadline:?}) while \
             {BURST} real login attempts were in flight — a single worker thread was busy \
             with Argon2"
        );
        assert_eq!(status, StatusCode::OK);
        probes += 1;
    }
    assert!(
        probes > 5,
        "the burst finished before this test could probe it meaningfully ({probes} probes); \
         make BURST larger or the guess wrong-er"
    );

    for h in handles {
        let _ = h.await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn d5_the_costliest_call_site_does_not_stall_a_health_check_either() {
    // RFC 126's own summary: a wrong recovery-code guess costs up to
    // RECOVERY_CODE_COUNT (8) sequential Argon2 verifications in
    // `match_recovery_code` — the worst single-request blocking cost in
    // the codebase. If any call site were still blocking a worker thread,
    // this is the one most likely to make it visible.
    let state = test_app();
    let session = complete_setup_and_login(&state).await;
    let _ = enroll_mfa_for(&state, &session).await;
    let per_probe_deadline = probe_deadline(calibrate_single_hash_cost().await);

    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::POST)
        .uri("/admin/login")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(format!(
            "username={USERNAME}&password={PASSWORD}"
        )))
        .expect("req");
    let resp = router.oneshot(req).await.expect("login");
    let pending = extract_set_cookie(resp.headers(), "sui_id_pending_mfa").expect("pending");
    let router = build_router(state.clone());
    let req = Request::builder()
        .method(Method::GET)
        .uri("/admin/login/mfa")
        .header(header::COOKIE, format!("sui_id_pending_mfa={pending}"))
        .body(Body::empty())
        .expect("req");
    let resp = router.oneshot(req).await.expect("mfa GET");
    let csrf = extract_set_cookie(resp.headers(), "sui_id_csrf").expect("csrf");

    const BURST: usize = 4;
    let mut handles = Vec::with_capacity(BURST);
    for _ in 0..BURST {
        let state = state.clone();
        let pending = pending.clone();
        let csrf = csrf.clone();
        handles.push(tokio::spawn(async move {
            let _ = build_router(state.clone())
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri("/admin/login/mfa")
                        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                        .header(
                            header::COOKIE,
                            format!("sui_id_pending_mfa={pending}; sui_id_csrf={csrf}"),
                        )
                        .body(Body::from(format!(
                            "code={}&_csrf={csrf}",
                            urlencode("NOT-ANY-OF-THE-REAL-RECOVERY-CODES")
                        )))
                        .expect("req"),
                )
                .await
                .expect("recovery-code guess");
        }));
    }

    // Same measurement shape as the login burst above, and for the same
    // reasons: wall clock around a yield-then-probe iteration, not a
    // `tokio::time::timeout` around the probe alone, and no tight loop with
    // no yield (which would starve the burst rather than race it). The
    // deadline is calibrated above, before the burst starts — a blocked
    // probe here would be queued behind up to 8 hashes per caller, not 1,
    // so the same floor is, if anything, more conservative in this test.
    let mut probes = 0usize;
    while handles.iter().any(|h| !h.is_finished()) {
        let t0 = std::time::Instant::now();
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let status = get_healthz(&state).await;
        let elapsed = t0.elapsed();
        assert!(
            elapsed < per_probe_deadline,
            "a probe iteration took {elapsed:?} (deadline {per_probe_deadline:?}) while \
             {BURST} wrong recovery-code guesses (up to 8 hashes each) were in flight"
        );
        assert_eq!(status, StatusCode::OK);
        probes += 1;
    }
    assert!(
        probes > 5,
        "the burst finished before this test could probe it meaningfully ({probes} probes)"
    );

    for h in handles {
        let _ = h.await;
    }
}
