# RFC 102 implementation handoff

**Governing RFC.** [RFC 102](../../accepted/102-authentication-fails-closed-without-audit.md),
Accepted 2026-09-17. The RFC is the contract; this file sets the order and the
evidence. **Implementer.** Mid-capability model.
**Reviews.** [design review](102-103-design-review-2026-09-16.md),
[confirmation review](102-103-confirmation-review-2026-09-17.md); its §2.4 lists
the `*_within_tx` variants each command needs.

## Order

Each stage is one review request. Stages 1–4 are dispatched in order. Stage 1 is
dispatched **now**.

| Stage | Content | Why here |
|---|---|---|
| **1** | **B7** (factor additions gated; first-factor re-authentication), **L06** (per-session step-up failure count, revocation at 5), the **step-up rate-limit bucket**, **B3** (recovery codes refused for step-up), `auth.mfa.factor_added` | Live defect: a stolen session can add a factor and pass step-up. Owner-authorized ahead of the rest, 2026-09-17 |
| 2 | **L01** with A7 (password path); A9 | R11's path |
| 3 | **L02** and **L07** (per-user second-factor count), N14's sign-in WebAuthn script | the replay races |
| 4 | **L03**, **L04** | external-source and federated paths |
| 5 | **A4** — `sessions::insert` crate-private; U30 retired | after every sign-in path is converted |
| 6 | **L05** — step-up success atomic; WebAuthn ceremony kind swap removed | |
| 7 | **B4** on the four sealed gated commands, including `not_applicable`; **B6** | |
| 8 | matrix classes and events; threat model; handoff closure | |

## Stage 1 — landed `04a5760`, 2026-09-17

Reviewed, and committed as one commit. Accepted in the review:
- **Event names.** U12, U14 and U15 now record `auth.mfa.factor_added` instead of
  the older names. The command inventory is updated.
- **Passkey registration route.** `webauthn.js` was posting to routes that no
  longer existed; that is fixed.
- **Step-up errors.** A store error during step-up now returns 400.

Carried forward:
- **LDAP re-bind username.** The re-bind uses the local username, which differs
  from the directory's when the shadow row was suffixed. This goes to
  `roadmap/ldap-returning-signin/`, which owns username mapping.
- **Federated users.** First-factor enrolment is refused until upstream
  re-authentication exists (RFC 096-B1).

## Stage 2 — landed `19344f4`, 2026-09-17

Reviewed, and committed as one commit. Refusing a user who was locked after the
password check (by a concurrent failure) is accepted: clearing that lock would
undo U22.

## Stage 3 — landed `b25929e`, 2026-09-17

Reviewed, and committed as one commit. Accepted:
- **L07 counts rejected ceremonies.** A missing or expired ceremony counts, as
  well as a bad signature. Only `Store` and `Internal` errors are excluded,
  which matches the step-up mapping.
- **The lock count stays after the lock.** A later wrong factor locks again, with
  a longer window.
- **A stricter `next` check on the passkey path.**
- **WebAuthn success is covered by reading only,** because no software
  authenticator is available. RFC 099's live-integration evidence must include
  a real passkey sign-in.

## Stage 4 — landed `3041d8a`, 2026-09-17

Reviewed, and committed as one commit. **Every sign-in path is now Class A.**

Accepted:
- **`auth.user_source.matched` is retired.** It had no other writer.
- **A7 is not applicable on the federated callback.** The callback has no
  post-authentication refusal, and `FedState.next` is stored but never read;
  recorded for RFC 096-B1.
- **A first directory sign-in resolves its id before the transaction.** A
  concurrent first sign-in loses as an ordinary 401.

Carried into stage 5 as follow-ups:
- **Evidence was lost.** L04 dropped the upstream `sub`, and retiring
  `auth.user_source.matched` dropped the directory `stable_id` from the audit
  trail. Both are needed to answer "which upstream identity signed in",
  especially after a link or shadow row changes. Neither is a secret.
- **`unwrap_or_default()` for a new `UserId`** is correct, because `Default` is a
  fresh v4, but it reads like a zero value in identity code.
- **Log-capture retries** in the append-failure tests work around tracing's
  callsite interest cache. That goes to `roadmap/request-id-span/`.

## Stage 5 — landed `0b03f3a`, 2026-09-17

Reviewed, and committed as one commit. **Part A is complete.** Every session is
created by L01–L04, and nothing outside the store can insert one; a
compile-negative fixture proves it. Accepted:
- `sessions::insert` is `#[cfg(test)] pub(crate)`.
- The runtime "no audit row" test for the Protocol runner moved to T09's test.

## Stage 6 — dispatched 2026-09-17: L05, step-up success atomic

**Baseline.** The commit that adds this section, or later.

**Scope** — RFC 102 Part B, B1 and B-F5:
- **L05** (sealed Class A) replaces `touch_step_up` on both success paths
  (`verify_totp_code`, `finish_webauthn`). In one transaction:
  - re-read the session: it exists, belongs to the user, is unrevoked and
    unexpired, and the user is active;
  - consume the factor:
    - for TOTP, the guarded `last_used_step` advance (reuse L02's statement);
    - for WebAuthn, a guarded delete of this user's `StepUp`-kind ceremony row
      (zero rows → roll back);
  - set `last_step_up_at` and `last_step_up_method`;
  - reset `step_up_failure_count`;
  - write `auth.step_up.success` with attributes `method` and `gate`. `gate` is
    the sanitised `return_to`, truncated with `truncate_utf8`.
- **B-F5: no kind swap.** `webauthn::finish_authentication` takes the expected
  ceremony kind. `start_webauthn` creates the row as `StepUp` directly, and
  `finish_webauthn` no longer rewrites it to `Authenticate`. The sign-in path
  passes `Authenticate`. The ceremony row is consumed inside L05 for step-up;
  on the sign-in path, say where it is consumed today and leave it there.
- **Remove `touch_step_up`,** or make it crate-private if a store test needs it.
  The two e2e callers in `crates/sui-id/tests/e2e/mfa.rs` reach freshness by
  stepping up through the handler, or through L05. Do not add a raw test route.
- **A9.** A failed L05 runs no L06, and gets the existing uniform step-up
  response: the 400 page for TOTP, JSON 400 for WebAuthn. The cause is logged.

**Evidence.**
- **Happy path**, TOTP and WebAuthn (WebAuthn through the command, since no
  authenticator is available): one event, fresh, the method recorded, the count
  reset.
- **Injected append failure:** no freshness, step or ceremony unchanged, count
  unchanged, the uniform response, the log line.
- **Concurrency:** one TOTP code on two concurrent step-ups gives exactly one
  success event and one freshness change.
- **Revalidation:** a session revoked between verification and commit is rolled
  back.
- **The ceremony kind:** a sign-in `Authenticate` ceremony cannot complete a
  step-up, and a `StepUp` ceremony cannot complete a sign-in. Test both
  directions against the new `finish_authentication` parameter.
- **Mutation:** the session re-read, the TOTP guard, the ceremony guard and the
  kind check, one at a time; each is caught.
- **Build and gates:** fmt, both clippy scopes, the test count (default and all
  features), MSRV 1.95, G13, G15.

## Stage 5 — as dispatched

**Baseline.** The commit that adds this section, or later.

**5a — stage 4 follow-ups.**
- **L04:** `auth.federation.signin.success` gains a bounded `sub` attribute (the
  upstream subject, truncated to 255 bytes), next to `provider` and `evicted`.
- **L03:** `auth.login.success` gains a bounded `stable_id` attribute on the
  directory path (truncated to 255 bytes; a DN is allowed).
- **Explicit ids.** Replace `expected_id.unwrap_or_default()` with an explicit
  `UserId::new()`, and require `expected_id` where the caller always has one.
- **Docs.** Update the matrix and `docs/src/reference/audit-events.md`
  attribute lists. G15 must pass.

**5b — A4.**
- **`sessions::insert` becomes `pub(crate)`** in `sui-id-store`, and so does
  `insert_within_tx` if nothing outside needs it.
- **Retire `commands::insert_session`** (U30's Protocol runner, no production
  caller), and correct its manifest row.
- **Test callers outside the store** — `crates/sui-id-core/src/authn/step_up.rs`
  tests, `authn/session.rs` tests and `crates/sui-id/tests/e2e/me_security.rs`
  (the design review's §2.6):
  - they create sessions by signing in through the commands;
  - or they use a store-side test constructor that runs **L01's transaction**.

  **No `test-support` feature may re-export raw access** (RFC 094 migration
  checklist). If a caller cannot use either route, stop and report.
- **Structural check.** A test or gate that fails if a production module outside
  `sui-id-store` names `sessions::insert`. A `pub(crate)` compile error is the
  mechanism; show the compile-negative fixture, as RFC 094's fixtures do.

**Evidence.**
- 5a: attribute tests for both events, including truncation.
- 5b: the compile-negative fixture, and the list of every former caller with
  the route it now takes.
- **Build and gates:** fmt, both clippy scopes, the test count (default and all
  features), MSRV 1.95, G13, G15.

## Stage 4 — as dispatched

**Baseline.** The commit that adds this section, or later. It builds on the LDAP
returning-sign-in package (`478ec5b`), which moved the directory path's session
code into `directory_session`.

**Scope** — RFC 102 Part A, paths 4 and 5:
- **L03** (sealed Class A) replaces `directory_session`'s no-MFA branch. In one
  transaction:
  - upsert the shadow user (the manifest's U26 row is corrected: L03 subsumes it);
  - re-read the user as active and not locked;
  - reset the password counter and stale lock;
  - set `last_login_at`;
  - insert the session and evict over the cap (L01's helper);
  - write `auth.login.success` with a bounded `source` attribute (the source
    slug).

  The best-effort `auth.user_source.matched` append, `set_last_login` and
  `clear_lockout` are removed. Keep `auth.user_source.matched` as a registered
  event only if something else still writes it; otherwise retire its matrix
  row and say so.
- **The shadow upsert on the MFA branch.** It commits before the pending row is
  issued. Moving it into L02 is not required; state what is written before the
  second factor.
- **L04** (sealed Class A) replaces `complete_federated_signin`'s no-MFA branch.
  In one transaction:
  - re-read the user as active (the fail-closed checks from `b2ecc5c` stay in
    front);
  - set `last_login_at`;
  - insert the session and evict over the cap;
  - write `auth.federation.signin.success` with the provider slug.

  The best-effort append after the session is removed.
- **A7 on both paths**, before any write. Once L03 refuses an admin-only
  destination, the directory cascade no longer needs `login_post`'s
  post-session role check; remove that check.
- **A9 on both paths.** A failed L03 or L04 is never counted, and gets the
  uniform response: 401 for the directory, the `fed_error=signin_failed`
  redirect for federation. The cause is logged.

**Out of scope, recorded, not changed.** Both paths keep their 24-hour session
lifetime, and the directory path keeps `amr: [fed]`. Both are RFC 102 findings
that need their own decision; do not change them here.

**Evidence.**
- **Happy path**, for each: one session, one Atomic event with its attribute,
  bookkeeping applied.
- **Injected append failure**, for each: no session, no shadow change (L03), no
  event; the uniform response; the log line.
- **Cap:** eviction on each path.
- **A7:** a non-admin through the directory to `/admin` has no session row.
- **Mutation:** the re-read, the eviction and A7's ordering on each path; each
  is caught.
- **Build and gates:** fmt, both clippy scopes, the test count (default and all
  features), MSRV 1.95, G13.

## Stage 3 — as dispatched

**Baseline.** The commit that adds this section, or later.

**Ruling on stage 2's question.** L02 takes over the reset of the password
counter and stale lock. The MFA branch of `login_with_mfa` no longer calls
`clear_lockout`. The counter resets only when the whole sign-in commits, so the
reset belongs to A2's transaction. RFC 102's L02 row is read to include it.

**Scope** — RFC 102 Part A, paths 2 and 3:
- **L02** (sealed Class A), in one transaction:
  - consume the pending-MFA row with a guarded delete (the row exists, belongs to
    the user and is unexpired; zero rows → roll back);
  - for TOTP, the guarded `last_used_step` advance (`WHERE last_used_step < ?`);
  - for a recovery code, the compare-and-swap on `recovery_codes_enc`;
  - re-read the user as active and not locked;
  - reset the password counter and stale lock, and the user's second-factor
    failure count;
  - set `last_login_at`;
  - set freshness **by method**: TOTP and WebAuthn set `last_step_up_at` and
    `last_step_up_method`; a recovery code sets neither (RFC 102 N5 and open
    question 3, ruled);
  - insert the session and evict over-cap sessions (reuse L01's helper);
  - event `auth.mfa.success` (method, evicted), now Atomic.
- **L07** (sealed Class A). Add a per-user second-factor failure count; a
  migration adds `users.mfa_failure_count`. At 5, delete every pending-MFA row
  for the user and lock the account with U22's backoff. Events are
  `auth.mfa.failure` (count) or `auth.mfa.lockout` (count, locked_for_secs).
- **Wiring.** `mfa_challenge_post` and the sign-in WebAuthn completion call L02.
  A wrong factor runs L07; a failed L02 does not (A9). Remove the best-effort
  `auth.mfa.success` and `auth.mfa.failure` appends.
- **A7 on the MFA path.** An admin-only destination for a user who cannot read
  the admin panel is refused before any pending row is issued.
- **N14.** The sign-in WebAuthn script follows the completion response, and a
  pending `next` survives. Every error on that step maps to `Unauthenticated`
  (a redirect to `/admin/login`).

**Evidence.**
- **Concurrency**, on a multi-thread runtime and repeated. Each case failed in
  every iteration at the design review:
  - one TOTP code on two pending rows gives one session;
  - one recovery code on two pending rows gives one session, with one code
    removed;
  - two completions of one pending row give one session;
  - steps N+1 and N written concurrently leave the stored step at N+1.
- **Injected append failure:** nothing changes (no session, pending row kept,
  step and codes unchanged, counters unchanged), and the response is uniform.
- **L07:** four wrong codes then a right one signs in, and the count resets. Five
  wrong codes lock the account, remove the pending rows and refuse the right
  code. Minting new pending rows with the password does not reset the count.
- **Freshness:** a TOTP sign-in is fresh; a recovery-code sign-in is not.
- **A7** on the MFA path.
- **N14:** a WebAuthn sign-in failure keeps `next`, and success lands on `next`.
- **Mutation:** remove each guard, the per-user count, and the method rule for
  freshness, one at a time; each is caught.
- **Build and gates:** fmt, both clippy scopes, the test count, MSRV 1.95, G13.

**Baseline.** The commit that adds this section, or later.

**Scope** — RFC 102 Part A, applied to path 1 only:
- **L01** as a sealed Class-A command in `crates/sui-id-store/src/commands.rs`. In
  one transaction:
  - clear the failure counter and any stale lock;
  - set `last_login_at`;
  - insert the session;
  - evict over-cap sessions (move `server_settings`, count, oldest and single
    revoke reads into `*_within_tx`; see review §2.4);
  - re-read the user as active.

  Event `auth.login.success`, with the evicted count as a bounded attribute. Its
  registry class becomes Atomic.
- **`login_with_mfa`'s no-MFA branch** calls L01 instead of `sessions::insert`,
  `clear_lockout`, `enforce_concurrent_session_cap`, `record_login_success` and
  `set_last_login`. The MFA branch keeps `clear_lockout` for now; stage 3 owns
  it. Say whether leaving it there changes any stage 3 assumption.
- **A7.** The admin-only-`next` refusal in `login_post` runs before L01, from the
  role, so no session or event is committed for a refused sign-in.
- **A9.** A correct password whose L01 fails runs no U22 and gets the uniform 401
  (R11 1c). The cause is logged (R11 1b).

**Evidence.**
- **Happy path:** one session, one Atomic event, counter cleared, `last_login_at`
  set.
- **Injected append failure** (RFC 094's seam, or the R11 audit-trigger harness):
  no session, no event, counter and lock unchanged, no eviction. The response is
  byte-identical to the 401 after normalisation, and the log line is present.
- **Cap:** with the cap at N, the (N+1)th sign-in leaves exactly N sessions,
  committed with its event.
- **A7:** a non-admin with an admin-only `next`: no session row, no event.
- **R11:** extend `r11_login_failure.rs` 1a. With the audit log failing, a
  correct password no longer signs in.
- **Mutation:** remove the in-transaction re-read, the eviction, and A7's
  ordering, one at a time; each is caught.
- **Build and gates:** fmt, both clippy scopes, test count before and after,
  MSRV 1.95, G13.

**Baseline.** The commit that adds this file, or later.

**Scope.**
- **Migration.** Add `sessions.step_up_failure_count INTEGER NOT NULL DEFAULT 0`
  and `sessions.last_step_up_method TEXT` (nullable). Stage 6 uses the second
  column, and adding both now avoids a second migration of `sessions`.
- **L06** as a sealed Class-A command. Increment the count; at 5, revoke that one
  session. Branch events are `auth.step_up.failure` (count) or
  `auth.step_up.session_revoked` (count). It needs `sessions::revoke_within_tx`
  for a single row.
- **The step-up rate-limit bucket.** Add `RateLimitKey::StepUp`. It applies to
  `POST /me/security/step-up`, `step-up/webauthn/start`, `step-up/webauthn/finish`,
  and every B7 re-authentication form.
- **Wire the failures.** A wrong code on any step-up path runs L06. A9 holds: a
  store error on the success path does **not** run L06. Keep the existing
  responses (RFC 102's failure table, step-up rows).
- **B3.** `verify_totp_code` stops accepting recovery codes, and the doc comment
  at `step_up.rs:89-90` is corrected (first-review L1).
- **B7 gates.** When the user has any second factor,
  `passkey_register_start`/`complete`, `mfa_regenerate_recovery` and
  `mfa_enroll_start`/`confirm` require `require_fresh_step_up`. When the user has
  none:
  - a local user re-enters the password on the form; a wrong password runs L06
    and uses the step-up bucket;
  - an LDAP user re-binds against the user source, and a failure counts the same;
  - a federated user re-authenticates upstream with `prompt=login` and
    `max_age=0`, and the returned `auth_time` must be later than the enrolment
    request. **If this needs federation plumbing you cannot add without touching
    RFC 096-B1's files, stop and report.** Until then, refuse first-factor
    enrolment for federated users, and say so in the UI.
  - If re-authentication is unavailable, enrolment is refused.
- **`auth.mfa.factor_added`** (method) is a Class-A event with the enrolment write
  for all three factor kinds. If a factor's enrolment write is not yet on the
  seam, convert that write in this stage (U12, U14 or U15 per the inventory) and
  name it.
- **Registry and matrix.** Register the new events in `registry.rs` and
  `ci/audit-coverage-matrix.md`. G13 must pass.

**Evidence.**
- **Stolen-session tests** (RFC 102 test plan, B7): a second client holding the
  cookie cannot register a passkey, regenerate codes or enrol TOTP without a
  fresh step-up. A local user with no factor cannot enrol without the password.
- **L06 tests.** Four failures then a success: the count resets and the session
  is kept. Five failures: the session is revoked and the next request is
  unauthenticated. The bucket refuses a burst. A store error on L05's
  predecessor path runs no L06 (A9).
- **N1 test.** Wrong passwords on the enrolment form are counted, and the fifth
  revokes the session.
- **B3.** A valid recovery code on the step-up form is refused, not consumed, and
  freshness is unchanged.
- **Mutation.** Remove each gate and each count in turn; each removal is caught.
- **Build and gates.** fmt, both clippy scopes, `cargo test --workspace` count
  before and after, MSRV 1.95, G13, G12 (new UI strings in en, ja and zh_hans).
