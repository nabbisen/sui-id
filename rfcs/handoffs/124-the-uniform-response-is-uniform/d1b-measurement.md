# RFC 124 D1b — stage 1 measurement: the six branches, timed

**Date:** 2026-10-01
**Baseline.** `05a1a18` (HEAD).
**Changes nothing.** No production code is touched by this file. A temporary
test file produced the numbers below and was deleted afterward (the
convention RFC 123's `d4-measurement.md` established); its full text is
reproduced at the bottom of this file so the measurement can be re-run from
any commit that includes RFC 124's current `forgot_password.rs`.

## Confirming the four claims, at this baseline

- **The claim.** `crates/sui-id-core/src/account/forgot_password.rs:32`:
  `request_reset` "takes roughly the same time in both branches." Confirmed —
  this is the line, verbatim, at `05a1a18`.
- **The known-address path** inserts a reset token at
  `forgot_password.rs:174` (`password_reset_tokens::insert(db, &row).await?;`)
  — confirmed, exact line. It also reads SMTP configuration (`:178`) and, on
  success, dispatches mail and writes further events, as the handoff states.
- **The unknown-address path** returns after one lookup
  (`users::find_by_email_normalized`, `:100`) and one event (`:121-128`) —
  confirmed.
- **The production mailer.** `crates/sui-id/src/main.rs:110-134`'s `serve()`
  — the only production entry point — calls `startup::prepare`, which builds
  `AppState` with an `OutboxMailSender` (`startup.rs:229-231`: "RFC 001: use
  the persistent outbox sender in production"). A *separate* `SmtpMailSender`
  is built only inside the `OutboxWorker` (`main.rs:122-133`), which is a
  background task, off the request path — it drains the outbox later, not
  during `request_reset`. **The handoff's `startup.rs:243` pointer is stale**
  (code has moved since the handoff was written; `:243` now falls inside the
  unrelated metrics-registry block) — the claim itself is confirmed, at
  `startup.rs:229-231` as of this baseline.
- **No deployment-configurable inline-SMTP alternative exists** for the
  request-serving `AppState`. `AppState::new` (`runtime/state.rs`) takes
  whatever `Arc<dyn MailSender>` its caller constructs, and `serve()` is the
  only caller that builds one for real traffic; it always builds
  `OutboxMailSender`, unconditionally, with no config flag selecting
  anything else. So `request_reset`'s `mailer.send(mail)` call, on the
  request path, in production, is always a local encrypt-and-insert
  (`outbox.rs`'s `encrypt_field` + `email_outbox::enqueue`) — never inline
  SMTP, never a real socket. This confirms the handoff's instruction that the
  send cost to measure is the local insert, not SMTP.

## Method

`request_reset` (`sui-id-core::forgot_password`) called directly, in-process,
against an in-memory SQLite database — no HTTP, no socket. The mailer is a
real `OutboxMailSender` over the same database (per the confirmation above —
the actual production mailer on the request path), not a stub. 300 timed
samples per branch after a 5-iteration warm-up, release profile
(`cargo test --release`). p10/median/max reported (not mean), matching
RFC 123's own measurement.

**Environment.** AMD Ryzen 9 9950X (16-core / 32-thread), Linux 7.2.8
(CachyOS), `rustc 1.98.1`, release profile, in-process (not through the real
HTTP path — see "Method notes and limitations" below for what that does and
doesn't license).

**Branches**, per the security review's six-rung enumeration (not the
original two). Each sampled independently; see the reproduction code at the
bottom for exactly how each is constructed:

| # | Branch |
|---|---|
| 1 | Address unknown |
| 2 | Found, non-Local source (seeded with `UserSource::Ldap`) |
| 3 | Found, Local, no credentials row — never activated |
| 4 | Found, Local, credentialed, disabled |
| 5 | Found, Local, credentialed, active, at the outstanding-token cap (3 pre-inserted tokens) |
| 6 | Found, Local, credentialed, active, under the cap — the full path: token mint, insert, SMTP-config read, real outbox insert |

Branches 1–5 reuse one seeded user across all 300 samples (none of them
mutate state that would move the user to a different branch on the next
call). Branch 6 necessarily uses a fresh user per sample: a successful call
increments that user's outstanding-token count, and the third such call
would divert the *next* call into branch 5 instead of branch 6.

## Results — three independent runs

| Branch | Run 1 p10/median/max | Run 2 p10/median/max | Run 3 p10/median/max |
|---|---|---|---|
| 1 — unknown | 16.8 / 19.3 / 129.8 µs | 20.3 / 23.9 / 51.2 µs | 21.4 / 22.2 / 87.8 µs |
| 2 — non-Local | 21.5 / 24.2 / 32.3 µs | 27.5 / 28.4 / 165.3 µs | 26.0 / 28.7 / 52.7 µs |
| 3 — never activated | 21.5 / 24.6 / 121.8 µs | 26.8 / 27.2 / 43.2 µs | 24.9 / 25.7 / 118.2 µs |
| 4 — disabled | 22.9 / 23.5 / 55.9 µs | 35.9 / 36.7 / 71.8 µs | 28.1 / 31.3 / 88.5 µs |
| 5 — at cap | 25.5 / 27.3 / 124.4 µs | 35.4 / 36.2 / 72.8 µs | 31.6 / 32.3 / 136.4 µs |
| 6 — under cap (real send) | 233.9 / 269.1 / 656.7 µs | 242.6 / 279.3 / 593.3 µs | 268.1 / 380.1 / 686.5 µs |

## What the numbers answer

**1. Is the difference distinguishable at all?** Yes, but **two very
different differences are bundled under "the difference," and they are not
remotely the same size.**

- **Branch 6 against everything else (1–5): yes, overwhelmingly.** Branch
  6's p10 (234–268 µs) exceeds branch 1–5's *max* in every one of the three
  runs. The real send (token mint, insert, outbox encrypt-and-insert) costs
  roughly **8–15× more wall-clock time** than any early-return branch's
  median. This is the same shape RFC 123's D4 measurement found for
  `authenticate_client` — a gap large enough that it needs no careful
  statistics to see over a real network, because it dwarfs ordinary
  scheduler and network jitter at the microsecond scale this was measured
  at.
- **Branches 1–5 against each other: yes, but barely.** Branch 1 (unknown)
  sits at 20–27 µs median across the three runs; branches 2–5 (one or two
  extra DB reads each) sit at 24–37 µs median. The gap between the cheapest
  (1) and the most expensive early-return branch (4 or 5, whichever reads
  the outstanding-token count) is on the order of **3–13 µs** — real,
  reproduced in every run, but two orders of magnitude smaller than the
  branch-6 gap.

**2. What would an attacker need?**

- **To tell "this address triggers a real send" (branch 6) from "this
  address does not" (branches 1–5, which covers unknown, non-Local,
  never-activated, disabled, and already-at-cap all at once): very little.**
  A gap of hundreds of microseconds is large relative to typical same-host
  or same-datacenter network jitter (tens to low hundreds of µs) and would
  need only a handful of samples there; it is small relative to typical
  wide-area internet jitter (often single-digit to double-digit
  milliseconds), where it would need on the order of
  `(jitter_std_dev / 300µs)²` samples per address to average out — for 1ms
  jitter, roughly a dozen; for 5ms jitter, a few hundred. **Either way this
  is a practical attack for a patient, scriptable caller, over almost any
  network**, and it is a strictly worse finding than RFC 124's original
  two-branch framing: it also separates disabled/deleted accounts (branch 4)
  and accounts already mid-recovery (branch 5) from active ones under the
  cap (branch 6), not merely "exists" from "does not."
- **To tell branches 1–5 apart from each other** (in particular, 3 against
  1 — the never-activated rung the security review calls sharpest): the
  signal is a few microseconds. Against same-host or same-rack jitter
  (tens of µs std dev), the sample count needed is on the order of
  `(jitter/5µs)²` — for 50 µs jitter, on the order of 100 samples per
  address. Against realistic wide-area jitter (single-digit milliseconds),
  the same formula gives tens of thousands to hundreds of thousands of
  samples per address — **impractical for a remote attacker**, and the
  existing per-IP throttle (five requests per sixty seconds, named as a
  residual risk in the security review) makes gathering even the
  LAN-feasible hundred samples slow (on the order of twenty minutes per
  address from a single IP). This part of the claim ("roughly the same
  time") is technically false but its practical exploitability is low
  except from an attacker already positioned close to the server, which is
  a materially different threat model than the one RFC 124 was written
  against.

**3. What would distinguish the narrow gap, if it matters.** Nothing in this
measurement rules out that a sufficiently patient, well-positioned, or
IP-rotating attacker could eventually separate branches 1–5 from each other.
A method that would settle it more conclusively: the same six branches,
timed over the real HTTP path (not in-process) from a fixed, repeatable
network vantage point, with enough samples to report a confidence interval
on the few-microsecond gap rather than a point estimate — the stage 2
dispatch should decide whether that additional work is worth doing *before*
or *instead of* the structural fix, given the conclusion below.

## Conclusion

**The claim at `forgot_password.rs:32` is false as written**, and the
security review's finding is the more useful frame: the six-branch ladder
has one large step (branch 6 against the rest — trivially exploitable) and
five small steps bunched together at its base (branches 1–5 against each
other — a real but narrow, low-practicality gap). **This does not argue for
tuning each of the five small branches to match — it argues for the
review's structural recommendation**: move all classification after the
response is written, so there is exactly one code path before the response
and none of these six branches' costs (large or small) run on the request
path at all. Equalizing six branches by work-matching each one (mirroring
RFC 123's dummy-hash approach) would require finding and maintaining five
separate matched costs forever, each one a place a future edit can
reintroduce a gap; the structural fix removes the possibility by
construction, for all six branches, in one change.

**This is not a recommendation to delete the claim and stop.** The
underlying property — a response that doesn't let a caller classify an
address — is real, worth having, and currently false in a way that is
cheaply exploitable for the large gap specifically. The claim's text is
wrong about why it's true today (there are not two branches, and even the
closest two are not equal), but the goal it describes is sound and the
security review already names the fix.

## Method notes and limitations

- Measured in-process, not over a real socket, per the dispatch's own
  allowance — "a large enough gap survives network jitter without careful
  statistics" — which applies cleanly to the branch-6 gap (order of
  magnitude) and not to the branches-1–5 gap (a few microseconds), as
  discussed above. The in-process numbers establish that a non-zero CPU/DB
  cost difference exists for the small gap; they do not by themselves
  establish real-world exploitability of that specific gap, which depends
  on an attacker's network position in the way quantified above.
- This file's numbers come from a temporary test added to this branch's
  working tree for the duration of the measurement and removed afterward —
  it is not part of the shipped diff. Reproduced in full below.
- Branch 6 uses a real `OutboxMailSender` (the actual production mailer on
  this path, confirmed above), not a stub — so its measured cost includes
  the real encrypt-and-insert, not an approximation of it.
- Not measured under concurrent load or against a live, loaded server —
  this is the same scope RFC 123's D4 measurement had: a single-caller
  timing-channel measurement, not a throughput or concurrency claim.

## Reproduction: the temporary test file

Added as `crates/sui-id/tests/e2e/rfc124_measurement.rs`, registered with a
`mod rfc124_measurement;` line in `crates/sui-id/tests/e2e/main.rs`, run with
`cargo test --release -p sui-id --test e2e rfc124_d1b_branch_timings -- --nocapture`,
then both the file and the `mod` line were removed. Full text:

```rust
//! RFC 124 D1b — stage 1 measurement, temporary.
//!
//! Not part of the shipped diff: added for this measurement, deleted
//! afterward (same convention RFC 123's D4 measurement used). The numbers
//! this file produces are recorded in
//! `rfcs/handoffs/124-the-uniform-response-is-uniform/d1b-measurement.md`,
//! along with this file's full text, so the measurement can be re-run from
//! any commit that includes RFC 124's current `forgot_password.rs`.
//!
//! Method: `request_reset` called directly, in-process, against an
//! in-memory SQLite database — no HTTP, no socket. 300 timed samples per
//! branch after a 5-iteration warm-up, release profile. The mailer is a
//! real `OutboxMailSender` over the same database (the production mailer
//! on the request path, per `crates/sui-id/src/main.rs:110-134`: `serve()`
//! wires `AppState` to `OutboxMailSender`, and only the separate
//! `OutboxWorker` — a background task, off the request path — holds an
//! `SmtpMailSender`), not an `InMemoryMailSender`, so branch 6's measured
//! cost includes the real local encrypt-and-insert.

use super::common::*;
use std::time::{Duration, Instant};
use sui_id_core::forgot_password::request_reset;
use sui_id_core::mail::outbox::OutboxMailSender;
use sui_id_shared::ids::{PasswordResetTokenId, UserId};
use sui_id_store::models::{
    CredentialRow, PasswordResetTokenRow, ResetTokenOrigin, Role, UserRow, UserSource,
};
use sui_id_store::repos::{credentials, password_reset_tokens, users};

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

async fn insert_active_token(
    state: &sui_id::AppState,
    user_id: UserId,
    now: chrono::DateTime<chrono::Utc>,
    salt: u8,
) {
    password_reset_tokens::insert(
        &state.db,
        &PasswordResetTokenRow {
            id: PasswordResetTokenId::new(),
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

#[tokio::test]
async fn rfc124_d1b_branch_timings() {
    let state = test_app();
    let outbox = OutboxMailSender::new(state.db.clone(), state.clock.clone());
    enable_smtp(&state).await;
    let issuer = state.config.server.issuer.clone();

    // Branch 1: unknown address.
    {
        let mut samples = Vec::with_capacity(N);
        for i in 0..(WARMUP + N) {
            let t0 = Instant::now();
            request_reset(
                &state.db,
                &state.clock,
                &outbox,
                &issuer,
                "nobody-unknown@example.test",
                None,
            )
            .await
            .expect("ok");
            let el = t0.elapsed();
            if i >= WARMUP {
                samples.push(el);
            }
        }
        let (p10, median, max) = stats(samples);
        println!("branch1_unknown p10={p10:?} median={median:?} max={max:?}");
    }

    // Branch 2: found, non-Local source (directory/upstream).
    {
        seed_user(
            &state,
            "ldap-user",
            "ldap-user@example.test",
            UserSource::Ldap,
            None,
            false,
        )
        .await;
        let mut samples = Vec::with_capacity(N);
        for i in 0..(WARMUP + N) {
            let t0 = Instant::now();
            request_reset(
                &state.db,
                &state.clock,
                &outbox,
                &issuer,
                "ldap-user@example.test",
                None,
            )
            .await
            .expect("ok");
            let el = t0.elapsed();
            if i >= WARMUP {
                samples.push(el);
            }
        }
        let (p10, median, max) = stats(samples);
        println!("branch2_nonlocal p10={p10:?} median={median:?} max={max:?}");
    }

    // Branch 3: found, Local, no credentials row — never activated.
    {
        seed_user(
            &state,
            "never-activated",
            "never-activated@example.test",
            UserSource::Local,
            None,
            false,
        )
        .await;
        let mut samples = Vec::with_capacity(N);
        for i in 0..(WARMUP + N) {
            let t0 = Instant::now();
            request_reset(
                &state.db,
                &state.clock,
                &outbox,
                &issuer,
                "never-activated@example.test",
                None,
            )
            .await
            .expect("ok");
            let el = t0.elapsed();
            if i >= WARMUP {
                samples.push(el);
            }
        }
        let (p10, median, max) = stats(samples);
        println!("branch3_never_activated p10={p10:?} median={median:?} max={max:?}");
    }

    // Branch 4: found, Local, credentialed, disabled.
    {
        seed_user(
            &state,
            "disabled-user",
            "disabled-user@example.test",
            UserSource::Local,
            Some("irrelevant-password-1"),
            true,
        )
        .await;
        let mut samples = Vec::with_capacity(N);
        for i in 0..(WARMUP + N) {
            let t0 = Instant::now();
            request_reset(
                &state.db,
                &state.clock,
                &outbox,
                &issuer,
                "disabled-user@example.test",
                None,
            )
            .await
            .expect("ok");
            let el = t0.elapsed();
            if i >= WARMUP {
                samples.push(el);
            }
        }
        let (p10, median, max) = stats(samples);
        println!("branch4_disabled p10={p10:?} median={median:?} max={max:?}");
    }

    // Branch 5: found, Local, credentialed, active, at the outstanding-token
    // cap. A successful call here doesn't mint, so the same user is safe to
    // reuse across all 300 samples.
    {
        let uid = seed_user(
            &state,
            "at-cap-user",
            "at-cap-user@example.test",
            UserSource::Local,
            Some("irrelevant-password-2"),
            false,
        )
        .await;
        let now = state.clock.now();
        for salt in 0..3u8 {
            insert_active_token(&state, uid, now, salt).await;
        }
        let mut samples = Vec::with_capacity(N);
        for i in 0..(WARMUP + N) {
            let t0 = Instant::now();
            request_reset(
                &state.db,
                &state.clock,
                &outbox,
                &issuer,
                "at-cap-user@example.test",
                None,
            )
            .await
            .expect("ok");
            let el = t0.elapsed();
            if i >= WARMUP {
                samples.push(el);
            }
        }
        let (p10, median, max) = stats(samples);
        println!("branch5_at_cap p10={p10:?} median={median:?} max={max:?}");
    }

    // Branch 6: found, Local, credentialed, active, under the cap — the
    // full path, including token mint/insert and a real send. A fresh user
    // per sample: a successful call here increments the user's
    // outstanding-token count, and a third successful call would push the
    // *next* call for the same user into branch 5 instead.
    {
        let mut samples = Vec::with_capacity(N);
        for i in 0..(WARMUP + N) {
            let email = format!("under-cap-{i}@example.test");
            seed_user(
                &state,
                &format!("under-cap-{i}"),
                &email,
                UserSource::Local,
                Some("irrelevant-password-3"),
                false,
            )
            .await;
            let t0 = Instant::now();
            request_reset(&state.db, &state.clock, &outbox, &issuer, &email, None)
                .await
                .expect("ok");
            let el = t0.elapsed();
            if i >= WARMUP {
                samples.push(el);
            }
        }
        let (p10, median, max) = stats(samples);
        println!("branch6_under_cap p10={p10:?} median={median:?} max={max:?}");
    }
}
```
