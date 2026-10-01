# RFC 124 stage 2 — the request path's cost, re-measured after the fix

**Date:** 2026-10-01
**Baseline:** working tree at stage 2's implementation (this package).
**Method, environment:** identical to stage 1's `d1b-measurement.md` — in-process, release profile, AMD Ryzen 9 9950X, `rustc 1.98.1`, 300 timed samples per branch after a 5-iteration warm-up, p10/median/max.

## What's timed

`sui_id_store::repos::forgot_password_requests::record` — the entirety of what `/forgot-password`'s handler now does that depends on `email` at all. (CSRF and rate-limit checks happen before this and are address-independent — not what RFC 124 is about — exactly as stage 1 excluded work past the point a real caller would already have a response.)

Same six branches as stage 1, same seeding. `record` does not read any of the seeded state — it is a single unconditional `INSERT` — so this measurement is expected to show what it shows: no branch left to separate.

## Results — three independent runs

| Branch | Run 1 p10/median/max | Run 2 p10/median/max | Run 3 p10/median/max |
|---|---|---|---|
| 1 — unknown | 15.2 / 25.0 / 90.1 µs | 11.3 / 12.2 / 121.7 µs | 16.2 / 26.9 / 122.3 µs |
| 2 — non-Local | 15.5 / 25.1 / 109.7 µs | 15.3 / 21.2 / 77.7 µs | 16.8 / 26.3 / 85.7 µs |
| 3 — never activated | 10.9 / 17.5 / 50.3 µs | 12.8 / 22.8 / 46.2 µs | 16.8 / 26.6 / 63.8 µs |
| 4 — disabled | 7.3 / 11.6 / 19.1 µs | 10.3 / 24.6 / 61.7 µs | 16.2 / 25.9 / 63.2 µs |
| 5 — at cap | 6.6 / 12.0 / 26.4 µs | 11.1 / 18.1 / 51.2 µs | 14.8 / 25.2 / 97.8 µs |
| 6 — under cap | 11.9 / 20.8 / 165.2 µs | 9.9 / 19.2 / 98.2 µs | 12.7 / 25.1 / 71.6 µs |

## Conclusion

**The branch-6 gap stage 1 found (an order of magnitude, ~234–380 µs vs ~19–37 µs) is gone.** Every branch's median now sits in the same 11–27 µs band across all three runs, and every branch's range overlaps every other branch's — the same "trivially indistinguishable" shape stage 1 found for the one branch pair (2 against 1) that was already safe by construction (RFC 103 D13).

This is the expected, and only interesting, result: `record` does not branch on anything the six scenarios differ by, so there is nothing left in the request path for a timing measurement to separate. The six-branch cost ladder stage 1 measured still exists — inside `request_reset`, now run by `ForgotPasswordWorker`, off the request path — but it is no longer reachable from the response, by construction, which is the structural property D1 asked for rather than a result that happens to hold today.

## Reproduction

Added as `crates/sui-id/tests/e2e/rfc124_stage2_measurement.rs`, registered with `mod rfc124_stage2_measurement;` in `crates/sui-id/tests/e2e/main.rs`, run with `cargo test --release -p sui-id --test e2e rfc124_stage2_branch_timings -- --nocapture`, then both removed — not part of the shipped diff, same convention stage 1 used. Full text:

```rust
//! RFC 124 stage 2 — the request path's cost, re-measured after the fix.
//!
//! Temporary, like stage 1's `d1b-measurement.md`: the numbers below are
//! recorded in `rfcs/handoffs/124-the-uniform-response-is-uniform/d2-measurement.md`
//! along with this file's full text, then this file is deleted. Same
//! method as stage 1: in-process, release profile, 300 timed samples per
//! branch after a 5-iteration warm-up, p10/median/max.
//!
//! What's timed is `forgot_password_requests::record` directly — the
//! entirety of what `/forgot-password`'s handler now does that depends on
//! `email` at all (CSRF and rate-limit checks, which come before this and
//! are address-independent, are not part of what RFC 124 is about and are
//! excluded, same as stage 1 excluded SMTP-config reads that happen after
//! the point a real HTTP caller would have already gotten a response).

use super::common::*;
use std::time::{Duration, Instant};
use sui_id_shared::ids::{ForgotPasswordRequestId, UserId};
use sui_id_store::models::{CredentialRow, PasswordResetTokenRow, ResetTokenOrigin, Role, UserRow, UserSource};
use sui_id_store::repos::{credentials, forgot_password_requests, password_reset_tokens, users};

const N: usize = 300;
const WARMUP: usize = 5;

async fn seed_user(
    state: &sui_id::AppState,
    username: &str,
    email: &str,
    source: UserSource,
    password: Option<&str>,
    disabled: bool,
) -> UserId {
    let now = chrono::Utc::now();
    let uid = UserId::new();
    users::create(
        &state.db,
        &UserRow {
            id: uid,
            username: username.into(),
            display_name: None,
            is_admin: false,
            role: Role::User,
            last_login_at: None,
            is_disabled: disabled,
            is_deleted: false,
            user_uuid: uuid::Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            failed_login_count: 0,
            locked_until: None,
            source,
            external_stable_id: None,
            email: Some(email.into()),
            preferred_lang: None,
            email_normalized: Some(sui_id_shared::normalize_email(email)),
            email_verified_at: None,
        },
    )
    .await
    .expect("create user");
    if let Some(password) = password {
        credentials::upsert(
            &state.db,
            &CredentialRow {
                user_id: uid,
                password_hash: sui_id_core::password::hash_password(password)
                    .await
                    .expect("hash"),
                updated_at: now,
            },
        )
        .await
        .expect("credentials");
    }
    uid
}

async fn insert_active_token(state: &sui_id::AppState, user_id: UserId, salt: u8) {
    let now = chrono::Utc::now();
    password_reset_tokens::insert(
        &state.db,
        &PasswordResetTokenRow {
            id: sui_id_shared::ids::PasswordResetTokenId::new(),
            user_id,
            token_hash: vec![salt; 32],
            issued_at: now,
            expires_at: now + chrono::Duration::minutes(30),
            consumed_at: None,
            requester_ip: None,
            issued_via: ResetTokenOrigin::Email,
            issued_by: None,
            revoked_at: None,
        },
    )
    .await
    .expect("insert token");
}

fn stats(mut xs: Vec<Duration>) -> (Duration, Duration, Duration) {
    xs.sort();
    let p10 = xs[xs.len() / 10];
    let median = xs[xs.len() / 2];
    let max = *xs.last().expect("non-empty");
    (p10, median, max)
}

async fn time_record(state: &sui_id::AppState, email: &str) -> Duration {
    let t0 = Instant::now();
    forgot_password_requests::record(
        &state.db,
        ForgotPasswordRequestId::new(),
        email.to_owned(),
        Some("203.0.113.1".into()),
        state.clock.now(),
    )
    .await
    .expect("record");
    t0.elapsed()
}

#[tokio::test]
async fn rfc124_stage2_branch_timings() {
    let state = test_app();

    // Branch 2: non-local.
    seed_user(&state, "ldap-user", "ldap-user@example.test", UserSource::Ldap, None, false).await;
    // Branch 3: never activated.
    seed_user(&state, "never-activated", "never-activated@example.test", UserSource::Local, None, false).await;
    // Branch 4: disabled.
    seed_user(&state, "disabled-user", "disabled-user@example.test", UserSource::Local, Some("irrelevant-1"), true).await;
    // Branch 5: at cap.
    let at_cap = seed_user(&state, "at-cap-user", "at-cap-user@example.test", UserSource::Local, Some("irrelevant-2"), false).await;
    for salt in 0..3u8 {
        insert_active_token(&state, at_cap, salt).await;
    }
    // Branch 6: under cap.
    seed_user(&state, "under-cap-user", "under-cap-user@example.test", UserSource::Local, Some("irrelevant-3"), false).await;

    let branches: [(&str, &str); 6] = [
        ("1_unknown", "nobody-unknown@example.test"),
        ("2_nonlocal", "ldap-user@example.test"),
        ("3_never_activated", "never-activated@example.test"),
        ("4_disabled", "disabled-user@example.test"),
        ("5_at_cap", "at-cap-user@example.test"),
        ("6_under_cap", "under-cap-user@example.test"),
    ];

    for (name, email) in branches {
        let mut samples = Vec::with_capacity(N);
        for i in 0..(WARMUP + N) {
            let el = time_record(&state, email).await;
            if i >= WARMUP {
                samples.push(el);
            }
        }
        let (p10, median, max) = stats(samples);
        println!("branch{name} p10={p10:?} median={median:?} max={max:?}");
    }
}
```
