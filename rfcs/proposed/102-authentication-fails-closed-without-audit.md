# RFC 102 — Authentication that cannot be audited does not succeed

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None beyond the owner rulings this RFC carries out:
- **R11 Part 2** (`@nabbisen`, 2026-09-16): fail closed.
- **Process** (same day): carry it in a new RFC rather than amend RFC 094 and
  RFC 096 in place (option (b)).
- **Step-up** (same day): the owner's instruction on step-up auditing — "Log is
  important. Consider carefully." It extended this RFC from sign-in to step-up
  re-authentication (Part B).

**Implementation prerequisites.** This RFC Accepted, and RFC 094 M2a's **runner
foundation** — `declare_write_command!`, `WriteTx<AtomicAudit>` and
`Database::class_a`. The foundation is already in the tree and carries U22.

**Closure prerequisites.** Every production path that establishes a session, or marks a session freshly stepped-up, commits that change and exactly one registered event in one transaction; no production caller of the raw session insert or the raw step-up touch remains; an injected audit-append failure on each path leaves nothing changed, and the user gets the uniform failure response; step-up failures are counted and throttled; every step-up-gated action's own event records the step-up evidence that authorized it; `docs/threat-model.md` states the resulting properties; independent closure review accepts the evidence.

**Tracks.** `ROADMAP.md` risk register row R11, Part 2. The step-up findings are
recorded in Part B's background.

**Touches.** `crates/sui-id-store/src/`: `commands.rs` (new commands), `repos/sessions.rs`, `repos/users.rs`, `repos/login_pending_mfa.rs`, `repos/webauthn_pending.rs`, `repos/user_totp.rs`, and a migration; `crates/sui-id-core/src/authn/`: `session.rs`, `mfa.rs`, `step_up.rs`, `webauthn.rs`; `crates/sui-id/src/http/`: `handlers.rs`, `handlers/step_up.rs`, `handlers/admin/auth.rs`, `handlers/admin/webauthn.rs`, `handlers/federation.rs`, and every step-up-gated handler; `ci/write-commands.toml`, `ci/audit-coverage-matrix.md`, `docs/threat-model.md`.

**Handoff.** Written on acceptance as
`rfcs/handoffs/102-authentication-fails-closed/README.md`. The design review
request is in the same directory.

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
acceptance, one-line pointers here go into:
- RFC 094's inventory rows U24, U30, U31 and U32;
- RFC 096's F01 and F03.

Their designs are otherwise unchanged. Until then, RFC 096-B1 must not implement
F01 or F03. That hold is recorded in the RFC 096 handoff's entry gates and in
`ROADMAP.md`.

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
| **L02** second-factor completion | 2, 3 | Consume the pending-MFA row with a guarded delete (zero rows → roll back, `Unauthenticated`). For TOTP, advance `last_used_step` with a guard (`WHERE last_used_step < step`; zero rows → roll back). For a recovery code, remove the used hash with a guard that it is still present. Set `last_login_at`, insert the session, evict over-cap sessions. | `auth.mfa.success`, with the method as a bounded attribute |
| **L03** external-source sign-in | 4 | Takes the shadow-user upsert result as input; U26 remains its own command. Set `last_login_at`, insert the session, evict over-cap sessions. | `auth.login.success`, with `source` as a bounded attribute |
| **L04** federated sign-in (shipped path) | 5 | Set `last_login_at`, insert the session, evict over-cap sessions. | `auth.federation.signin.success` |

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

- **The append or any statement fails:** roll back and return the step's normal
  failure:
  - password step: 401 with the uniform message;
  - second-factor step: the MFA failure response.

  No session cookie is set, and the failure is logged (C1).
- **The password is correct but the transaction fails:** the failure counter does
  **not** advance, because no wrong credential was presented. The stale lock is
  not cleared.
- **The TOTP step, L02.** Today `set_last_used_step` is an unconditional `UPDATE`,
  committed before the session is written. It runs after `totp::verify` compared
  the code against a step read earlier. So one code, submitted on two pending rows
  at once, can pass on both. Inside L02 the guarded update makes one of them roll
  back, which closes that replay race. A code whose sign-in fails to commit stays
  usable within its window, so the user's retry succeeds (*Open question 2*).
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
  step-up-gated Class-A command carries a required `step_up` attribute, in one of
  two forms:
  - `fresh`, with the method and the seconds since the step-up;
  - `not_required`, with reason `no_second_factor`.

  The attribute is computed by the gate from the session row it read. The
  descriptors make it required, so a gated command cannot commit without it.
- **B5 — uniform failure.** A correct factor whose transaction fails gets the
  same response as a wrong factor, and its code or ceremony is not consumed.
- **B6 — no bypass.** The raw step-up touch has no production caller outside L05.

### Commands

| Command | Transaction contents | Events (closed branches) |
|---|---|---|
| **L05** step-up success | Re-read the session: it must exist, be unrevoked and unexpired, and belong to the user. Consume the factor: for TOTP, the guarded `last_used_step` advance; for WebAuthn, a guarded delete of the `StepUp`-kind ceremony row for this user (zero rows → roll back). Set `last_step_up_at` and the new `last_step_up_method`, and reset the session's step-up failure count. | `auth.step_up.success`, with attributes `method` (`totp` \| `webauthn`) and `gate`, the sanitized `return_to` path truncated to 256 bytes |
| **L06** step-up failure | Increment the session's step-up failure count. At 5, revoke that session. | `auth.step_up.failure` (count) **or** `auth.step_up.session_revoked` (count) |

The migration adds `last_step_up_method` and `step_up_failure_count` to
`sessions`.

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

Whether a stronger, type-level proof is wanted is *Open question 1*. The same
question applies to B4's evidence value.

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
2. **L02**: both second factors, including recovery-code and TOTP-step rollback.
3. **L03** and **L04**.
4. **A4**: `sessions::insert` crate-private; manifest and structural gate
   updated.
5. **Migration, L05, L06**, the step-up rate-limit bucket, and B3's
   recovery-code refusal.
6. **B4**: the `step_up` attribute on every gated command's descriptor; B6's
   visibility change.
7. `ci/audit-coverage-matrix.md` classes and new events; `docs/threat-model.md`
   states the properties (through RFC 097's baseline if that has landed first,
   otherwise directly); the RFC 094 and RFC 096 pointers are added.

Each step can be reviewed on its own, and each leaves unconverted paths as they
were.

## Test plan

For **each** of L01–L05:
- **Happy path:** one committed change and one event.
- **Injected append failure** (RFC 094's failure-injection seam): nothing
  changed. No session, freshness, counter or lock change; no pending row, TOTP
  step, recovery code or ceremony consumed; no eviction. The HTTP response is
  byte-identical to that step's ordinary failure, and the C1 log line is emitted.

**Concurrency.**
- Two completions of one pending row give exactly one session.
- One TOTP code on two pending rows gives exactly one session.
- One TOTP code on two concurrent step-ups gives exactly one freshness change.

**Revalidation.**
- A user disabled between verification and commit: rolled back.
- A session revoked between step-up verification and commit: rolled back.

**Step-up throttling (L06).**
- Four failures, then a success: the count resets and the session is kept.
- Five failures: the session is revoked, the next request is unauthenticated, and
  `auth.step_up.session_revoked` is written.
- The rate-limit bucket refuses a burst from one address.

**B3.** A valid, unused recovery code on the step-up form: refused, not consumed,
freshness unchanged.

**B4.**
- For every gated command, a structural test that its descriptor requires
  `step_up`.
- An end-to-end case for each form: a fresh TOTP step-up, and a no-MFA account.

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
- A dangerous action is already a Class-A command and cannot commit unaudited.
- Accounts with no second factor still pass step-up gates without a challenge
  (the design recorded in `step_up.rs`). B4 now makes every such pass visible in
  the audit log.

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

## Open questions

1. **Type-level proof of authentication.** Should the commands, and B4's evidence
   value, require a sealed proof value that only the verification functions can
   construct, instead of a caller list plus the structural gate? That is
   stronger, but the proof type would have to live in `sui-id-store` or in a crate
   both sides depend on. Goes to `@nabbisen` if the design reviewer cannot
   adjudicate it.
2. **A rolled-back TOTP step.** Is it acceptable that a code stays usable after a
   failed commit? The alternative is to advance the step in a separate committed
   write even when the operation fails. The recommendation is the former, because
   the latter burns a user's valid code on a server fault.
3. **Recovery-code sign-in and freshness.** Today L02's predecessor sets
   `last_step_up_at = now` for every second factor, recovery codes included. So
   signing in with a recovery code grants step-up freshness, which RFC 089's rule
   forbids in spirit. Honouring the rule leaves a user who lost their only
   authenticator unable to pass the gate on `/me/security/mfa`, so recovery goes
   through an administrator's MFA reset instead. **Recommended: honour RFC 089.**
   A recovery-code sign-in sets no freshness, and lost-authenticator recovery is
   the admin MFA reset. The design reviewer should first confirm which self-service
   MFA actions are gated. If a sole administrator could be left with no path, the
   question goes to `@nabbisen` together with RFC 103's CLI recovery.
