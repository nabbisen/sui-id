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

## Stage 3 — dispatched 2026-09-17: L02 and L07, the second factor

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
