# RFC 102 — Authentication that cannot be audited does not succeed

**Status.** Implemented
**Closure reviewed on.** 2026-09-22
**Closure approved by.** `@nabbisen` (accountable owner), 2026-09-22, who also
ruled prerequisite 6 (below). The closure review was performed by the architect,
which wrote this RFC and every dispatch, and is **not** independent of it. No
independent role existed: the implementation role built it, and `@nabbisen`
approved its design and acceptance. As with RFCs 093 and 098, recorded under
RFC 000 as an **unreviewed judgment carried by the owner**, not as a completed
independent review.
**Closure evidence.** [Closure assessment 2026-09-22](../handoffs/102-authentication-fails-closed/README.md)
(§*Closure assessment*, with the per-stage records above it: `19344f4`, `b25929e`,
`3041d8a`, `0b03f3a`, `742fe08`, `042b6dd`, `589a1d5`, `4698af9`, and stage 1's
`04a5760`). Every stage was reviewed against hash-pinned diffs with mutation
testing; CI green on each.
**Prerequisite 6, as ruled 2026-09-22 (`@nabbisen`).** "Every step-up-gated
action's own event records the step-up evidence" is read as **every action this
RFC converts** — U02, U03, U04, U07 and K01. Eight gated actions remain
unconverted and still append best-effort: client disable, delete and
rotate-secret; signing-key delete; self-service MFA disable and passkey delete;
revoke-all-other-sessions; applying a pending SMTP change. They gain B4 when
RFC 094 converts them, which RFC 102 B4 already requires; the ruling changes no
code and hides nothing.
**Accepted on.** 2026-09-17
**Approved by.** `@nabbisen`, who also ruled every open question as recommended.
**Independent design review.** [Design review 2026-09-16](../handoffs/102-authentication-fails-closed/102-103-design-review-2026-09-16.md)
by the implementation role, which authored neither RFC (one blocker, six high, eight medium, six low), and
[confirmation review 2026-09-17](../handoffs/102-authentication-fails-closed/102-103-confirmation-review-2026-09-17.md)
(three high, seven medium, four low, on the resolutions). Every finding is
resolved in this text. It checked implementability against the runner, every
session-insert and credential-writer call site, the gated call sites, and the
failure responses, and demonstrated the replay races by test.
**Implementation owner.** Mid-capability model, by dispatch.
**Security review.** Required
**Design prerequisites.** None beyond the owner rulings this RFC carries out:
- **R11 Part 2** (`@nabbisen`, 2026-09-16): fail closed.
- **Process** (same day): carry it in a new RFC rather than amend RFC 094 and
  RFC 096 in place (option (b)).
- **Step-up** (same day): the owner's instruction on step-up auditing — "Log is
  important. Consider carefully." It extended this RFC from sign-in to step-up
  re-authentication (Part B).
- **Independent design review** (implementation role, 2026-09-16; see the
  metadata above): one
  blocker, six high and eight medium findings. Every one is resolved in this text;
  the table under *Design review resolutions* maps each finding to where. A
  **confirmation review** of those resolutions (2026-09-17) found three high and
  seven medium issues in the new design (N1–N14), also resolved here.

**Implementation prerequisites.** This RFC Accepted, and RFC 094 M2a's **runner
foundation** — `declare_write_command!`, `WriteTx<AtomicAudit>` and
`Database::class_a`. The foundation is already in the tree and carries U22.

**Closure prerequisites.** Every production path that establishes a session, or marks a session freshly stepped-up, commits that change and exactly one registered event in one transaction; no production caller of the raw session insert or the raw step-up touch remains; an injected audit-append failure on each path leaves nothing changed, and the user gets the uniform failure response; step-up failures are counted and throttled; every step-up-gated action's own event records the step-up evidence that authorized it; `docs/threat-model.md` states the resulting properties; independent closure review accepts the evidence.

**Tracks.** `ROADMAP.md` risk register row R11, Part 2. The step-up findings are
recorded in Part B's background.

**Touches.** `crates/sui-id-store/src/`: `commands.rs` (new commands), `repos/sessions.rs`, `repos/users.rs`, `repos/login_pending_mfa.rs`, `repos/webauthn_pending.rs`, `repos/user_totp.rs`, and a migration; `crates/sui-id-core/src/authn/`: `session.rs`, `mfa.rs`, `step_up.rs`, `webauthn.rs`; `crates/sui-id/src/http/`: `handlers.rs`, `handlers/step_up.rs`, `handlers/admin/auth.rs`, `handlers/admin/webauthn.rs`, `handlers/federation.rs`, and every step-up-gated handler; `ci/write-commands.toml`, `ci/audit-coverage-matrix.md`, `docs/threat-model.md`.

**Handoff.** [`../handoffs/102-authentication-fails-closed/README.md`](../handoffs/102-authentication-fails-closed/README.md)

**Accountable owner and approver.** `@nabbisen`.

**RFC author / architect.** High-capability model, requirements-architect role.

**Independent security and closure reviewer.** Role independence per RFC 000:
the reviewer must not have authored, implemented, or previously approved this
RFC; vendor is not a criterion. Design review goes to the implementation role,
for implementability and for any gaps it would hit while building. The judgments
named in *Open questions* go to `@nabbisen` where no other role can adjudicate
them, and are recorded as unreviewed design judgment.

## Summary

Two authentication acts in sui-id are recorded best-effort or not at all:

- **Sign-in.** Every sign-in path writes its session first and its audit row
  afterwards, discarding any error. U22 does fail when the audit log cannot be
  written, so wrong passwords stop being counted. An audit-log failure therefore
  gives an attacker unlimited, uncounted guesses, then an unrecorded sign-in.
- **Step-up re-authentication** is the fresh second factor that gates every
  dangerous action. Nothing records it. Nothing counts or throttles its failures,
  so a stolen session can guess TOTP codes without limit. It also accepts
  recovery codes, which RFC 089 forbids.

This RFC applies one rule to both: **an authentication act that cannot be
audited does not take effect.** Establishing a session, and marking a session
freshly stepped-up, each become a Class-A command: the change and its event
commit together or not at all. Step-up failures are counted per session and
throttled per address. Every action a step-up gates records, in its own atomic
event, what authorized it:
- a fresh factor, with its method and age; or
- no challenge, because the account has no second factor.

## Why a new RFC, and how it relates to RFCs 094, 096 and 089

RFC 094 converts *existing* behaviour onto a typed, transactional seam. It
classifies sign-in as Protocol state with a Class-B success event (inventory
U24, U30, U32), and step-up bookkeeping as Protocol (U31). RFC 096 builds
federated sign-in on that classification (F01, F03). This RFC changes those
classifications, and adds a required attribute to the events of step-up-gated
commands. That is new behaviour with its own threat analysis, not a conversion.

RFC 095 is the precedent. It tightens RFC 094's `client.dynamic_register` on the
same seam without reopening RFC 094, and RFC 094 says so. This RFC does the same.
**Where this RFC differs from RFC 094 or RFC 096 on how a session is established
or stepped up, or on what a step-up-gated event carries, this RFC governs.** On
acceptance, one-line pointers here went into RFC 094's command inventory
(`rfcs/handoffs/094-transactional-audit/command-inventory.md`), rows U24, U30,
U31 and U32, and rows F01 and F03, the RFC 096 commands that inventory holds.
Neither RFC's body is edited.

Their designs are otherwise unchanged. RFC 096-B1 builds F01 and F03 to this RFC.
The hold that kept them from starting until acceptance is released, as recorded
in the RFC 096 handoff.

RFC 089 (done) already requires that recovery codes do not satisfy step-up. The
code does not comply. Part B restores the contract; RFC 089 is not edited.

## Part A — sign-in

### Background: the paths measured on 2026-09-16

Five production paths establish a session. All five insert the session through
the raw repository function, outside any audit transaction, then write their
audit row with `let _ = audit::append(...)`:

| # | Path | Session written at | Success event, as written today |
|---|---|---|---|
| 1 | Local password, no MFA | `crates/sui-id-core/src/authn/session.rs`, `login_with_mfa` | `auth.login.success`, best-effort, after `sessions::insert`. `clear_lockout` and `set_last_login` are also separate and best-effort. |
| 2 | TOTP or recovery-code second factor | `crates/sui-id-core/src/authn/mfa.rs`, `verify_pending` | `auth.mfa.success`, best-effort. The HTTP handler `admin/auth.rs` `mfa_challenge_post` writes it after the session exists. The pending-MFA row is deleted best-effort. |
| 3 | WebAuthn second factor | `authn/mfa.rs`, `verify_pending_webauthn` | `auth.mfa.success` (`note: webauthn`), best-effort, in `admin/webauthn.rs`. The pending row is deleted best-effort. |
| 4 | External user source (LDAP cascade) | `crates/sui-id/src/http/handlers/admin/auth.rs`, `try_login_with_cascade` | **None.** Only `auth.user_source.matched`, best-effort, before the session. There is no sign-in success event at all. |
| 5 | Shipped federation callback | `crates/sui-id/src/http/handlers/federation.rs`, `complete_federated_signin` | `auth.federation.signin.success`, best-effort, after the session |

Two paths *continue* a sign-in without creating a session: local password with
MFA enrolled, and federation with local MFA. Both write
`auth.login.password_ok_mfa_required` best-effort.

### Requirements

- **A1 — no unaudited session.** A committed session row created by a sign-in
  implies exactly one committed success event for it, and the converse. Both or
  neither: RFC 094 invariant A1, applied to sign-in.
- **A2 — the whole sign-in is one transaction.** Everything a successful sign-in
  changes commits with the session and the event, or rolls back with them:
  - clearing the failure counter and a stale lock;
  - `last_login_at`;
  - consuming the pending-MFA row;
  - evicting sessions over the concurrent-session cap.
- **A3 — uniform failure.** When the transaction fails, the response is exactly
  the response for a failed sign-in at that step (R11 ruling (a), 2026-09-16).
  Nothing else distinguishes it.
- **A4 — no bypass.** The raw session insert has no production caller. Sessions
  for sign-in are created only through this RFC's commands, and the command
  manifest and the structural gate say so.
- **A5 — every path.** Paths 1–5, and RFC 096's F01 and F03 when built.
- **A7 — no session nobody holds.** A sign-in that will be refused after
  authentication (today, a non-admin signing in to an admin-only `next`, in
  `login_post`) is refused **before** L01 or L03, from the role read outside the
  transaction: for L03, a new shadow user's role is always `user`, so an
  admin-only `next` is refused before the cascade runs (N7). Neither command
  commits a session and success event that no cookie carries.
- **A8 — second-factor failures are counted per user.** A wrong TOTP code,
  recovery code or WebAuthn assertion at the sign-in second factor commits a
  counted failure on the **user** (L07), not on the pending row. A per-row count
  is bypassed by minting a new pending row with each correct password (N2). After
  **5** consecutive failures, every pending-MFA row for the user is deleted and
  the account is locked with U22's backoff. By then the attacker has the
  password, so locking is correct, and the lock is audited. A successful L02
  resets the count.
- **A9 — a correct factor never counts as a failure.** When L02 or L05 fails to
  commit, L07 or L06 is **not** run. A user with the right code during a partial
  outage keeps their pending row and their session (N4).
- **A6 — continuations stay Class B.** Issuing a pending-MFA continuation grants
  no session and no authority, so its event stays Class B (`emit_must_attempt`).
  A sign-in cannot complete without passing A1 at its final step.

### Commands

There is one Class-A command per path. Each has a closed event enum and is
declared with `declare_write_command!` beside U22 in
`crates/sui-id-store/src/commands.rs`. The prefix is `L`, because RFC 094's
inventory already uses `S01`–`S10` for settings.

| Command | Replaces path | Transaction contents | Event |
|---|---|---|---|
| **L01** password sign-in | 1 | Clear the failure counter and stale lock; set `last_login_at`; insert the session; evict over-cap sessions. | `auth.login.success` |
| **L02** second-factor completion | 2, 3 | Consume the pending-MFA row with a guarded delete (zero rows → roll back, `Unauthenticated`). For TOTP, advance `last_used_step` with a guard (`WHERE last_used_step < step`; zero rows → roll back). For a recovery code: the codes are one sealed JSON blob (`user_totp.recovery_codes_enc`), so the guard is a compare-and-swap on the ciphertext, `UPDATE … SET recovery_codes_enc = ?new WHERE user_id = ? AND recovery_codes_enc = ?old` (zero rows → roll back); the Argon2 match runs outside. Set `last_login_at`; reset the user's second-factor failure count. **Freshness by method:** TOTP and WebAuthn set `last_step_up_at` and `last_step_up_method`; a recovery code sets neither (N5, *Open question 3*). Insert the session, evict over-cap sessions. | `auth.mfa.success`, with the method as a bounded attribute |
| **L03** external-source sign-in | 4 | Upsert the shadow user **inside** the transaction. U26 is listed in the manifest as an implemented command, but none exists (`users::upsert_ldap_shadow` is a raw function); L03 subsumes it and the manifest row is corrected. Set `last_login_at`, insert the session, evict over-cap sessions. | `auth.login.success`, with `source` as a bounded attribute |
| **L04** federated sign-in (shipped path) | 5 | Re-read the user: active, not deleted. **The local-MFA decision comes from a successful read**; a read error refuses the sign-in (today `is_mfa_enabled(..).unwrap_or(false)` skips MFA on error). Set `last_login_at`, insert the session, evict over-cap sessions. | `auth.federation.signin.success` |

| **L07** second-factor failure | 2, 3 | Increment the user's second-factor failure count; at 5, delete every pending-MFA row for the user and lock the account with U22's backoff. | `auth.mfa.failure` (count) **or** `auth.mfa.lockout` (count, locked_for_secs) |

RFC 096's F01 and F03 are built as L04's successors, under the same rule.

Credential verification (Argon2, TOTP, WebAuthn, the upstream assertion) stays
**outside** the transaction, as it is now, so no write lock is held across slow
or external work. The transaction re-reads what verification relied on and rolls
back if any of it changed:
- the user must still be active;
- for L02, the pending row must still exist and belong to the same user.

WebAuthn's own signature-counter update (U29) is unchanged.

### Event vocabulary: no renames

No sign-in event is renamed, so dashboards, filters, exports and the
audit-event-labels package keep working. What changes is each event's **class**
in the registry: Atomic instead of Class B. L03 is the one path that gains an
event it never wrote.

### Failure handling

- **The append or any statement fails:** roll back and return that step's
  ordinary failure, as fixed here. No session cookie is set, and the failure is
  logged (C1).

  | Step | Ordinary failure, and the response for a failed transaction |
  |---|---|
  | Password (paths 1, 4) | 401 login page with the uniform flash (today's `login_post` `Err` arm) |
  | TOTP or recovery code (path 2) | 401 challenge page with the uniform flash (today's `mfa_challenge_post` `Err` arm) |
  | WebAuthn second factor (path 3) | 303 redirect to `/admin/login` (`HttpError::html(Unauthenticated)`). **Today a store error maps to 500**; every error on this step maps to `Unauthenticated`. The sign-in script must follow the response rather than always navigating to `/admin` (N14) |
  | Federation callback (path 5) | Redirect to `/admin/login?fed_error=signin_failed`. **Today a store error maps to 500.** No template renders `fed_error` today; the login page shows one uniform federated-sign-in failure message for every value |
  | Step-up TOTP (L05/L06) | 400 step-up page with the invalid-code flash. **Today a store error maps to 500** |
  | Step-up WebAuthn | the uniform JSON 400 `{"error":"step_up_failed"}` the fetch endpoint already returns for every error, which `static/step-up-webauthn.js` consumes (N6) |
  | Second-factor lockout (L07 at 5) | the next request finds no pending row: redirect to `/admin/login` |
  | A7 refusal | the existing admin-only refusal response, before any write |
  | B4 freshness lost at commit | redirect to `/me/security/step-up?return_to=…`, as the gate does (N11) |

  **"Identical" means identical after removing per-response values**: the
  `request_id` in error pages and the CSRF token in challenge pages. Tests
  normalise exactly those two and compare the rest byte for byte.
- **The password is correct but the transaction fails:** the failure counter does
  **not** advance, because no wrong credential was presented. The stale lock is
  not cleared.
- **The TOTP step, L02.** Today `set_last_used_step` is an unconditional `UPDATE`,
  committed before the session is written. It runs after `totp::verify` compared
  the code against a step read earlier. So one code, submitted on two pending rows
  at once, can pass on both. Inside L02 the guarded update makes one of them roll
  back, which closes that replay race. The guard also stops the stored step
  moving **backwards**: today, if concurrent verifications accept steps N+1 and N
  and write in that order, the stored step becomes N and the N+1 code verifies
  once more. A code whose sign-in fails to commit stays usable within its window,
  so the user's retry succeeds (*Open question 2*, agreed by the review).
- **A consumed recovery code** also rolls back with a failed L02. Otherwise a
  storage fault would silently burn a single-use code.

### Session-cap eviction moves inside

Today `enforce_concurrent_session_cap` runs after the insert and absorbs its own
errors, so a committed sign-in can leave the user over the cap. Inside the
transaction, a committed session never exceeds it. Eviction gets no separate
event; the success event carries the evicted count as a bounded attribute.

RFC 074 introduced `set_last_login` as a best-effort helper, and its call site in
`authn/session.rs` says a failed write "must never abort login". That described a
separate write. Inside one transaction, a failing write means the database is
failing, and the sign-in fails with it. This RFC supersedes that choice; RFC 074,
being done, is not edited.

## Part B — step-up re-authentication

### Background: measured on 2026-09-16

Step-up is the fresh second factor required shortly before a dangerous action
(`crates/sui-id-core/src/authn/step_up.rs`; gate `require_fresh_step_up` in
`crates/sui-id/src/http/handlers.rs`). It currently gates 17 call sites:
- admin user, client, signing-key and SMTP-settings actions;
- self-service session revocation, MFA management and passkey deletion.

Freshness is `sessions.last_step_up_at` within `STEP_UP_FRESHNESS_SECS` (300).

- **B-F1: success is not recorded.** `touch_step_up` updates the session row. No
  code writes an `auth.step_up.*` event, and the matrix registers none. The
  dangerous-operations guide told operators to cross-check such rows; RFC 098
  dispatch 15 removes that instruction.
- **B-F2: failure is not recorded, counted or throttled.** `POST
  /me/security/step-up` (`crates/sui-id/src/http/handlers/step_up.rs`) calls no
  `enforce_rate_limit`. A wrong code returns 400 and changes nothing. Someone who
  holds a stolen session can submit TOTP codes without limit, and each attempt
  leaves no trace.
- **B-F3: recovery codes satisfy step-up, against RFC 089.** RFC 089 states
  "Recovery codes: Not accepted for step-up". `verify_totp_code` accepts a
  recovery code, consumes it, and sets `last_step_up_at`.
- **B-F4: the TOTP replay race** is the same one as Part A: an unconditional
  `set_last_used_step` after verification.
- **B-F5: the WebAuthn step-up ceremony changes kind by delete-then-insert,**
  twice (`start_webauthn`, `finish_webauthn`), as separate writes.
- **B-F6: dangerous actions do not record what authorized them.** Their Class-A
  events carry the operator and the target. They do not record whether a fresh
  factor authorized the action, or whether it passed with no challenge because
  the account has no second factor (`StepUpDecision::Allow` for a user without
  MFA).
- **B-F7: a stolen session can make itself fresh.** Found by the design review.
  Passkey registration (`passkey_register_start`, `passkey_register_complete`),
  recovery-code regeneration (`mfa_regenerate_recovery`) and TOTP enrolment
  (`mfa_enroll_start`, `mfa_enroll_confirm`) need only a session and CSRF. With a
  stolen cookie, register the attacker's passkey, step up with it, and every gated
  action is open. Or regenerate recovery codes and step up with one (B-F3).
  **Without closing this, nothing else in Part B stops a session thief.**
- **B-F8: most gated actions are not Class-A commands.** Of the 12 gated handlers
  that write, only four run a sealed command: `users_set_disabled` (U02/U03),
  `users_delete` (U04), `users_mfa_reset` (U07) and `signing_keys_rotate` (K01).
  The other eight mutate, then append best-effort, although the manifest marks
  several as implemented Class A:
  - client disable, delete and rotate-secret (C05, C06, C07);
  - signing-key hard delete (no manifest row at all);
  - self-service MFA disable (U13) and passkey delete (U16);
  - revoke-all-others (U19);
  - SMTP pending-change apply (S08 with S06).

### Requirements

- **B1 — no unaudited freshness.** A committed change to `last_step_up_at`
  implies exactly one committed `auth.step_up.success` event for it, and the
  converse.
- **B2 — failures are counted, throttled and recorded.** Each failed step-up
  attempt commits a counted failure event. After **5** consecutive failures on
  one session, that session is revoked in the same transaction. Step-up also
  gets its own per-address rate-limit bucket.
- **B3 — only phishing-resistant or time-based factors.** TOTP codes and WebAuthn
  assertions satisfy step-up. Recovery codes do not (RFC 089).
- **B4 — the action carries its authorization.** The event of every
  step-up-gated command carries a required `step_up` attribute, in one of two
  forms:
  - `fresh`, with the method and the seconds since the step-up;
  - `not_required`, with reason `no_second_factor`;
  - `not_applicable`, with reason `system_principal` — **only** for a command run
    through `for_system_actor` (the CLI or a scheduled trigger), and constructible
    only by that path. A session-bound branch can never produce it (N3). This
    covers U07 from RFC 103 D12's CLI, U37's CLI branch and K01's
    `system_principal` declaration.

  **The command computes it, not the gate.** A gated command takes the session ID
  as an input, re-reads the session row inside its own transaction, and derives
  the attribute from `last_step_up_at` and `last_step_up_method`. If the session
  is no longer fresh at commit, and the user has a second factor, the command
  rolls back, and the handler redirects to step-up (N11). No value crosses the handler, so nothing can be forged, and the gap
  between the gate's check and the commit is closed. The session ID is an input
  only; it never enters the event.

  **B4 applies only to sealed commands.** Each of the eight actions in B-F8 gains
  the attribute when RFC 094 converts it. Until then its row stays best-effort,
  and this RFC says so; it does not claim otherwise.
- **B7 — adding a factor requires proof.** Registering a passkey, regenerating
  recovery codes and enrolling TOTP are step-up-gated whenever the user already
  has any second factor. For a user with **no** second factor, the first factor's
  enrolment requires:
  - **a local user:** the current password, re-entered on the enrolment form. A
    wrong password is a step-up failure: it runs **L06** (the per-session count,
    and revocation at 5) and uses the step-up rate-limit bucket, with the uniform
    step-up failure response. Otherwise the form would be an unthrottled password
    oracle for a stolen session (N1). It is not U22, because account lockout
    would let the thief lock the real user out;
  - **an LDAP user:** the directory password, verified by re-binding against the
    user source, counted the same way;
  - **a federated user:** a fresh upstream authentication (`prompt=login`,
    `max_age=0`), whose `auth_time` must be later than the enrolment request.

  If re-authentication is unavailable (the directory or the provider is
  unreachable), enrolment is refused. There is no weaker fallback (N10).

  Enrolment of a first factor, and each later factor addition, is recorded:
  `auth.mfa.factor_added` (method), Class A with the enrolment write.
- **B5 — uniform failure.** A correct factor whose transaction fails gets the
  same response as a wrong factor, and its code or ceremony is not consumed.
- **B6 — no bypass.** The raw step-up touch has no production caller outside L05.

### Commands

| Command | Transaction contents | Events (closed branches) |
|---|---|---|
| **L05** step-up success | Re-read the session: it must exist, be unrevoked and unexpired, and belong to the user. Consume the factor: for TOTP, the guarded `last_used_step` advance; for WebAuthn, a guarded delete of the `StepUp`-kind ceremony row for this user (zero rows → roll back). Set `last_step_up_at` and the new `last_step_up_method`, and reset the session's step-up failure count. | `auth.step_up.success`, with attributes `method` (`totp` \| `webauthn`) and `gate`, the sanitized `return_to` path truncated to 256 bytes |
| **L06** step-up failure | Increment the session's step-up failure count. At 5, revoke that session. | `auth.step_up.failure` (count) **or** `auth.step_up.session_revoked` (count) |

The migration adds `last_step_up_method` and `step_up_failure_count` to
`sessions`, and `failure_count` to `login_pending_mfa` (L07).

L05 needs a guarded, single-row session revoke and touch; L06 needs
`sessions::revoke_within_tx` for one row. The review lists every
`*_within_tx` variant L01–L07 need (its §2.4); the handoff carries that list.

WebAuthn step-up stops swapping ceremony kinds (B-F5). `finish_authentication`
takes the expected kind as a parameter, and L05 consumes the row.

Verification stays outside the transaction, as in Part A.

### How a failure closes

Step-up follows the same argument as sign-in. While the audit log cannot be
written, L05 cannot commit, so no guess can succeed. And because a correct code
gets the same response as a wrong one (B5), a guess learns nothing even when L06
also fails to commit. Nothing is gained by guessing during an outage.

### Why revoke the session, not lock the account

Five wrong factors in a row from a signed-in session most likely mean a stolen
session. Revoking that session removes the thief and costs the real user one
sign-in. Locking the account would let the thief lock the real user out. The
password-lockout counter (U22) is not touched, because a step-up is not a
password attempt.

### What goes into the audit row, and what never does

- **Never:** the session identifier. It is the session cookie's value, a bearer
  secret. The audit row must not carry it, even hashed, because the audit log is
  readable by auditors.
- **Never:** a code, an assertion, or any part of one.
- **Always:** the user as actor and as target, the method, the gate path, the
  count.

## Common to both parts

### Authority for the context

U22 constructs its context with `for_system_actor(None)`, because nobody
authenticated. Commands L01–L05 *have* an actor: the user who just proved a
factor. But the store crate cannot see the core crate's verification, so no
store-side type can prove that verification happened. The minimum this RFC
requires:

- The commands take the verified `UserId` and method as inputs. They are callable
  only from `sui-id-core`'s authentication modules and the federation, cascade and
  step-up handlers. The command manifest's caller list and the structural gate
  enforce this.
- `sessions::insert` and `sessions::touch_step_up` become crate-private to
  `sui-id-store`.

A type-level proof is not required (*Open question 1*, resolved): every caller is
in the workspace, and the manifest plus the structural gate suffice, as for U22.
B4 avoids the question by computing its evidence inside the command.

`commands::insert_session` (U30, the Protocol runner) has no production caller and
is retired with A4. The four test callers of `sessions::insert` outside the store
crate sign in through the commands, or use a store-side test constructor that
creates a session **through L01's transaction**. A test feature that re-exports
raw access is not allowed (RFC 094 migration checklist).

### C1 — operational logs

The audit log records what happened. The operational log is where an operator
sees what failed. Every command in this RFC emits a structured `tracing` event:
- at **error** level when its transaction rolls back for any reason other than a
  guard that is expected to lose (an already-consumed row, a used code);
- at **warn** level on `auth.step_up.session_revoked`.

Fields: the request ID, the command ID, the user ID and the method. It never
carries a credential, a code or a session identifier. This is the operator-facing
half of A3 and B5: the response is uniform, and the log is where the cause goes.

## Multiple implementation steps

1. **L01**, with A4's visibility change deferred. This is the path R11 is about.
2. **L02 and L07**: both second factors, including recovery-code and TOTP-step
   rollback, the per-user failure count, and the sign-in WebAuthn script following
   the completion response (N14).
3. **L03** and **L04**.
4. **A4**: `sessions::insert` crate-private; manifest and structural gate
   updated.
5. **B7 first**, because nothing else in Part B holds without it: factor
   enrolment gated, and `auth.mfa.factor_added`.
6. **Migration, L05, L06, L07**, the step-up rate-limit bucket, and B3's
   recovery-code refusal.
7. **B4** on the four sealed gated commands; B6's visibility change. Each of the
   eight B-F8 actions gains B4 in the same commit that RFC 094 converts it.
8. `ci/audit-coverage-matrix.md` classes and new events; `docs/threat-model.md`
   states the properties (through RFC 097's baseline if that has landed first,
   otherwise directly); the RFC 094 and RFC 096 pointers are added.

Each step can be reviewed on its own, and each leaves unconverted paths as they
were. **The implementation order is the handoff's.** It starts with B7, which the
owner authorized on 2026-09-17 as a fix for a live defect ahead of the rest.

## Test plan

For **each** of L01–L05:
- **Happy path:** one committed change and one event.
- **Injected append failure** (RFC 094's failure-injection seam): nothing
  changed. No session, freshness, counter or lock change; no pending row, TOTP
  step, recovery code or ceremony consumed; no eviction. The HTTP response is
  byte-identical to that step's ordinary failure, and the C1 log line is emitted.

**Concurrency.** Multi-thread runtime, repeated; the design review demonstrated
the first three failing in every iteration today.
- One TOTP code on two pending rows gives exactly one session.
- One recovery code on two pending rows gives exactly one session, and exactly one
  code is removed.
- Two completions of one pending row give exactly one session.
- One TOTP code on two concurrent step-ups gives exactly one freshness change.
- Steps N+1 then N written concurrently: the stored step stays N+1.

**Revalidation.**
- A user disabled between verification and commit: rolled back.
- A session revoked between step-up verification and commit: rolled back.

**Step-up throttling (L06).**
- Four failures, then a success: the count resets and the session is kept.
- Five failures: the session is revoked, the next request is unauthenticated, and
  `auth.step_up.session_revoked` is written.
- The rate-limit bucket refuses a burst from one address.

**B7.** With a stolen session (a second client holding the cookie):
- a passkey cannot be registered, recovery codes cannot be regenerated, and TOTP
  cannot be enrolled while any factor exists without a fresh step-up;
- a local user with no factor cannot enrol one without the current password.

**L07.** Four wrong codes then a right one: signed in. Five wrong codes: the
pending row is gone, and the right code is refused.

**A7.** A non-admin signing in to an admin-only `next`: no session row, no success
event.

**B3.** A valid, unused recovery code on the step-up form: refused, not consumed,
freshness unchanged.

**B4.**
- For every gated command, a structural test that its descriptor requires
  `step_up`.
- An end-to-end case for each form: a fresh TOTP step-up, and a no-MFA account.
- Freshness expiring between the gate and the commit: the command rolls back.

**No bypass (A4, B6).** Structural fixtures: no production caller of the raw
session insert or step-up touch outside the commands.

**R11 end-to-end.** With the audit log failing:
- wrong passwords and a correct password both get the uniform 401, and no session
  is created;
- wrong and correct step-up codes both get the uniform failure, and no freshness
  is set.

## Security considerations

**The attacks this closes.**
- *Sign-in:* cause or wait for an audit-log write failure, guess passwords with no
  lockout (U22 fails), then sign in with the right one, unrecorded.
- *Step-up:* with a stolen session, guess TOTP codes with no throttle and no
  record until one works, then perform a dangerous action whose audit row does
  not show how it was authorized. After this RFC, guessing is throttled, counted
  and recorded, and ends with the session revoked. Success is recorded, and the
  action's own row says what authorized it.

**The cost is availability, stated plainly.** While the audit log cannot be
written, nobody can sign in or step up. Audit rows, sessions and step-up state
share one SQLite database, so most causes of an audit-write failure also break
the other writes, and those operations would have failed anyway. The cases that
remain are exactly the ones an attacker exploits:
- the audit chain's own constraints;
- a fault specific to `audit_log`.

C1 makes the outage visible to the operator instead of silent.

**What does not change.**
- Existing sessions keep working during an audit outage; this RFC governs
  establishing and stepping up a session, not using one.
- Dangerous actions: four already run as Class-A commands and cannot commit
  unaudited. The other eight (B-F8) still append best-effort until RFC 094 converts
  them. This RFC does not change that, and does not claim to.
- Accounts with no second factor still pass step-up gates without a challenge
  (the design recorded in `step_up.rs`). B4 now makes every such pass visible in
  the audit log.

**Residual in B7.** None of the first-factor paths trusts the session alone; each
re-proves the primary credential. A thief who holds the session **and** the
password can still enrol a factor. That is the password-compromise case, which a
second factor exists to limit, and requiring a second factor for all users is its
remedy; that is outside this RFC. Every enrolment is recorded
(`auth.mfa.factor_added`).

**Threat model.** `docs/threat-model.md` is the single home for security claims
(RFC 098 rule 7). This section records the decision; that document states the
resulting properties.

## Findings recorded while scoping, not in scope

Measured by reading on 2026-09-16. Each needs its own decision, not this RFC's:

- Path 4 labels an LDAP-sourced session `amr: [fed]`, although no federation took
  place.
- Paths 4 and 5 set a 24-hour session lifetime directly. Paths 1–3 use
  `SESSION_LIFETIME_HOURS`, which is 12, so external-source and federated sessions
  last twice as long, and no decision records why.
- The settings log summary and the dashboard count only `auth.login.success`.
  Sign-ins completed through a second factor or federation are not counted as
  sign-ins.
- **Found by the design review, each needing its own fix now:**
  - **A returning LDAP user cannot sign in** (H3). The shadow row takes the typed
    username, so the second sign-in finds a local user without a credential and
    never consults the directory. Path 4 works once per user.
  - **The shipped federation callback issues sessions to disabled or deleted
    users** (H4). They are refused at `/admin` and `/oauth2/authorize`, but `/me/*`
    accepts them. L04 closes this; the live defect needs a fix before L04.
  - **An LDAP or federated user can get a local password through forgot-password**
    (H5), bypassing the directory. RFC 103 records the control; the live defect
    needs a fix now.
  - Federation provisioning stores federated users with `source = ldap`
    (`federation.rs:504-511`).
  - The request-id middleware holds a span guard across `.await`
    (`crates/sui-id/src/http/request_id.rs`), so handler log lines can lose their
    `request_id` (found by R11 Part 1).

## Design review resolutions

| Finding | Resolved in |
|---|---|
| B1 stolen session enrols a factor | B-F7, **B7**, step 5, tests |
| H1 most gated actions not Class A | B-F8, B4 scope, step 7, *Security considerations* |
| H2 replay races; blob CAS; backward step | L02 row, *Failure handling*, concurrency tests |
| H3 returning LDAP user | *Findings* (live defect, own fix) |
| H4 federation MFA fail-open; disabled users | L04 row; *Findings* (live defect, own fix) |
| H5 external users get local passwords | *Findings*; RFC 103 T10 |
| H6 reset token in URL | RFC 103 D10 |
| M1 byte-identical; 500s | *Failure handling* table |
| M2 evidence computed by the command | B4 |
| M3 session nobody holds | A7 |
| M4 second-factor failures uncounted | A8, L07 |
| M5–M8 | RFC 103 |
| M6 U26 not a command | L03 row |
| L6 `sessions::insert` test callers; U30 retired | *Authority for the context* |

### Confirmation review (N1–N14)

| Finding | Resolved in |
|---|---|
| N1 B7 password re-entry is an oracle | B7 (L06 count, step-up bucket) |
| N2 per-row count bypassed | A8, L07 (per user; lock at 5) |
| N3 required `step_up` on CLI branches | B4 third form `not_applicable` |
| N4 correct factor counted as a failure | A9 |
| N5 L02 freshness by method | L02 row |
| N6 two table rows wrong; missing rows | *Failure handling* table |
| N7 A7 misses L03 | A7 |
| N8, N9, N12, N13 | RFC 103 D3, D10 |
| N10 session age weaker than re-auth | B7 (directory re-bind, upstream re-auth, no fallback) |
| N11 B4 rollback response | B4, table |
| N14 sign-in WebAuthn script ignores the response | table row, path 3; fixed in step 2 |
| First-review L1 (`step_up.rs` doc comment), L4 (cascade treats any error as unknown) | L1 in step 6 with B3; L4 with the H3 fix |

## Open questions

1. **Type-level proof of authentication. Resolved (design review).** Not required
   for L01–L07; B4 computes its evidence inside the command. See *Authority for the
   context*.
2. **A rolled-back TOTP step. Agreed by the design review.** Is it acceptable that a code stays usable after a
   failed commit? The alternative is to advance the step in a separate committed
   write even when the operation fails. The recommendation is the former, because
   the latter burns a user's valid code on a server fault.
3. **Recovery-code sign-in and freshness. Ruled 2026-09-17 (`@nabbisen`): as
   recommended — a recovery-code sign-in sets no freshness; lost-authenticator
   recovery is the administrator's MFA reset, with RFC 103 D12's CLI reset for a
   sole administrator.** Today L02's predecessor sets
   `last_step_up_at = now` for every second factor, recovery codes included. So
   signing in with a recovery code grants step-up freshness, which RFC 089's rule
   forbids in spirit. Honouring the rule leaves a user who lost their only
   authenticator unable to pass the gate on `/me/security/mfa`, so recovery goes
   through an administrator's MFA reset instead. **Recommended: honour RFC 089.**
   A recovery-code sign-in sets no freshness, and lost-authenticator recovery is
   the admin MFA reset. The design reviewer should first confirm which self-service
   MFA actions are gated. If a sole administrator could be left with no path, the
   question goes to `@nabbisen` together with RFC 103's CLI recovery.

   **Design review:** agreed, **but only together with B7** — without B7, passkey
   registration still yields freshness. With B7, lost-authenticator recovery is the
   administrator's MFA reset, and **a sole administrator who loses every factor has
   no path**. That goes to `@nabbisen` as RFC 103 D12.
