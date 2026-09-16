# RFC 102 — A sign-in that cannot be audited does not succeed

**Status.** Proposed
**Security review.** Required
**Design prerequisites.** None beyond the owner ruling this RFC implements:
R11 Part 2, ruled by `@nabbisen` on 2026-09-16 — *fail closed* — and the same
day's process ruling that it is carried by a new RFC rather than by amending
RFC 094 and RFC 096 in place (option (b)).
**Implementation prerequisites.** This RFC Accepted; RFC 094 M2a **runner
foundation** — `declare_write_command!`, `WriteTx<AtomicAudit>` and
`Database::class_a` — which is already in the tree and carries U22.
**Closure prerequisites.** Every production path that establishes a session
commits the session and exactly one registered success event in one transaction,
and no production caller of the raw session insert remains; an injected
audit-append failure on each path leaves no session, no cleared lockout, no
consumed pending row and the uniform failure response; `docs/threat-model.md`
states the rule; independent closure review accepts the evidence.
**Tracks.** `ROADMAP.md` risk register row R11, Part 2.
**Touches.** `crates/sui-id-store/src/commands.rs` (new sign-in commands),
`crates/sui-id-store/src/repos/sessions.rs`, `repos/users.rs`,
`repos/login_pending_mfa.rs`; `crates/sui-id-core/src/authn/session.rs`,
`authn/mfa.rs`; `crates/sui-id/src/http/handlers/admin/auth.rs`,
`admin/webauthn.rs`, `federation.rs`; `ci/write-commands.toml`,
`ci/audit-coverage-matrix.md`; `docs/threat-model.md`.
**Handoff.** Written on acceptance, as
`rfcs/handoffs/102-sign-in-fails-closed/README.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Independent security and closure reviewer.** Role independence per RFC 000 —
the reviewer must not have authored, implemented, or previously approved this
RFC; vendor is not a criterion. Design review routes to the implementation role
for implementability and for gaps it would hit while building. The two judgments
named in *Open questions* route to `@nabbisen` where no other role can
adjudicate them, recorded as unreviewed design judgment.

## Summary

Today every sign-in path writes its session first and its audit row afterwards,
best-effort: if the audit write fails, the user is signed in and nothing records
it. Combined with U22 — which *does* fail when the audit log cannot be written,
and so stops counting wrong passwords — an audit-log failure gives an attacker
unlimited, uncounted guesses followed by an unrecorded sign-in.

This RFC makes establishing a session a **Class-A command**: the session, the
sign-in bookkeeping and the success event commit together or not at all. If the
event cannot be written, there is no session, and the user sees the same failure
as any other failed sign-in.

## Why a new RFC, and how it relates to RFCs 094 and 096

RFC 094 converts *existing* behaviour onto a typed, transactional seam, and it
classifies sign-in as Protocol state with a Class-B success event (inventory U24,
U30, U32). RFC 096 builds federated sign-in on that classification (F01, F03).
This RFC changes the classification — new behaviour with its own threat
analysis, not a conversion.

RFC 095 set the precedent: it tightens RFC 094's `client.dynamic_register` on
the same seam without reopening RFC 094, and RFC 094 says so. This RFC does the
same for sign-in. **Where this RFC and RFC 094 or RFC 096 differ on how a session
is established, this RFC governs.** On acceptance, RFC 094's inventory rows U24,
U30 and U32 and RFC 096's F01 and F03 gain a one-line pointer here; their
designs are otherwise unchanged.

Until then, RFC 096-B1 must not implement F01 or F03 — recorded in the RFC 096
handoff's entry gates and in `ROADMAP.md`.

## Background — the paths measured on 2026-09-16

Five production paths establish a session. All five insert the session through
the raw repository function outside any audit transaction, then write their audit
row with `let _ = audit::append(...)`:

| # | Path | Session written at | Success event, as written today |
|---|---|---|---|
| 1 | Local password, no MFA | `crates/sui-id-core/src/authn/session.rs`, `login_with_mfa` | `auth.login.success`, best-effort, after `sessions::insert`; `clear_lockout` and `set_last_login` also separate and best-effort |
| 2 | TOTP or recovery-code second factor | `crates/sui-id-core/src/authn/mfa.rs`, `verify_pending` | `auth.mfa.success`, best-effort, written by the HTTP handler `admin/auth.rs` `mfa_challenge_post` after the session exists; the pending-MFA row is deleted best-effort |
| 3 | WebAuthn second factor | `authn/mfa.rs`, `verify_pending_webauthn` | `auth.mfa.success` (`note: webauthn`), best-effort, in `admin/webauthn.rs`; pending row deleted best-effort |
| 4 | External user source (LDAP cascade) | `crates/sui-id/src/http/handlers/admin/auth.rs`, `try_login_with_cascade` | **none** — only `auth.user_source.matched`, best-effort, before the session; no sign-in success event at all |
| 5 | Shipped federation callback | `crates/sui-id/src/http/handlers/federation.rs`, `complete_federated_signin` | `auth.federation.signin.success`, best-effort, after the session |

Two paths *continue* a sign-in without a session — local password with MFA
enrolled, and federation with local MFA — and write
`auth.login.password_ok_mfa_required` best-effort.

## Requirements

- **R1 — no unaudited session.** A committed session row created by a sign-in
  implies exactly one committed success event for it, and the converse. Both or
  neither (RFC 094 invariant A1, applied to sign-in).
- **R2 — the whole sign-in is one transaction.** Everything a successful sign-in
  changes commits with the session and the event, or rolls back with them:
  clearing the failure counter and stale lock, `last_login_at`, consumption of the
  pending-MFA row, and eviction of sessions over the concurrent-session cap.
- **R3 — uniform failure.** When the transaction fails, the response is exactly
  the response for a failed sign-in on that step — the R11 ruling (a) of
  2026-09-16. The failure is logged at error level with the request span and no
  credential material (R11 Part 1b), and nothing else distinguishes it.
- **R4 — no bypass.** The raw session insert has no production caller. Sessions
  for sign-in are created only through the commands this RFC defines; the
  command manifest and the structural gate say so.
- **R5 — every path.** Paths 1–5, and RFC 096's F01 and F03 when built.
- **R6 — continuations stay Class B.** Issuing a pending-MFA continuation grants
  no session and no authority; its event stays Class B (`emit_must_attempt`). A
  sign-in cannot complete without passing through R1 at its final step.

## Design

### Commands

One Class-A command per path — `L` for sign-in, since RFC 094's inventory already uses `S01`–`S10` for settings — each with a closed event enum, declared with
`declare_write_command!` beside U22 in `crates/sui-id-store/src/commands.rs`:

| Command | Replaces path | Transaction contents | Event |
|---|---|---|---|
| **L01** password sign-in | 1 | clear failure counter and stale lock; set `last_login_at`; insert session; evict over-cap sessions | `auth.login.success` |
| **L02** second-factor completion | 2, 3 | consume the pending-MFA row with a guarded delete (zero rows → roll back, `Unauthenticated`); for TOTP, advance `last_used_step` with a guard (`WHERE last_used_step < step`; zero rows → roll back); for a recovery code, remove the used hash with a guard that it is still present; set `last_login_at`; insert session; evict over-cap sessions | `auth.mfa.success`, with the method as a bounded attribute |
| **L03** external-source sign-in | 4 | shadow-user upsert result is an input (U26 remains its own command); set `last_login_at`; insert session; evict over-cap sessions | `auth.login.success`, with `source` as a bounded attribute |
| **L04** federated sign-in (shipped path) | 5 | set `last_login_at`; insert session; evict over-cap sessions | `auth.federation.signin.success` |

RFC 096's F01 and F03 are built as L04's successors on the same rule.

Credential verification — Argon2, TOTP, WebAuthn, the upstream assertion — stays
**outside** the transaction, as now: no write lock is held across slow or
external work. The transaction re-reads what verification relied on and rolls
back if it changed: the user is still active, and for L02 the pending row still
exists and belongs to the same user. WebAuthn's own signature-counter update
(U29) and ceremony consumption (U33) are unchanged by this RFC.

### Event vocabulary — unchanged

No event is renamed or added, so dashboards, filters, exports and the audit-event
labels package keep working. What changes is each event's **class** in the
registry: Atomic instead of Class B. L03 is the one path that gains an event it
never wrote.

### Authority for the context

U22 constructs its context with `for_system_actor(None)`, because nobody
authenticated. A sign-in *has* an actor — the user who just proved a credential —
but the store crate cannot see the core crate's verification, so a store-side
type cannot prove that verification happened. The minimum this RFC requires:

- the four commands take the verified `UserId` and the authentication methods as
  inputs, and are callable only from `sui-id-core`'s authentication modules and
  the two federation and cascade handlers — enforced by the command manifest's
  caller list and the structural gate;
- `sessions::insert` becomes crate-private to `sui-id-store`.

Whether a stronger, type-level proof is wanted is *Open question 1*.

### Failure handling

- **Append fails, or any statement fails:** roll back, return the step's normal
  failure (password step: 401 with the uniform message; second-factor step: the
  MFA failure response), log at error level. No session cookie is set.
- **Correct password but the transaction fails:** the failure counter is **not**
  advanced — no wrong credential was presented — and the stale lock is not
  cleared.
- **The TOTP step on L02:** today `set_last_used_step` is an unconditional
  `UPDATE` committed before the session, after `totp::verify` compared the code
  against the step it read earlier. Two submissions of one code racing on two
  pending rows can both pass. Inside L02 the guarded update makes one of them
  roll back — this closes that replay race. A code whose sign-in fails to commit
  stays usable within its window, so the user's retry succeeds (*Open question 2*).
- **A consumed recovery code** likewise rolls back with a failed L02 — otherwise a
  storage fault would silently burn a single-use code.

### Session-cap eviction moves inside

`enforce_concurrent_session_cap` currently runs after the insert and absorbs its
own errors, so a committed sign-in can exceed the cap. Inside the transaction a
committed session never exceeds it. Eviction is not given a separate event; the
success event carries the evicted count as a bounded attribute.

RFC 074 introduced `set_last_login` as a best-effort helper, and the call site in
`authn/session.rs` says a failed write "must never abort login". That described a
separate write; inside one transaction a failing write means the database is
failing, and the sign-in fails with it. This RFC supersedes that choice; RFC 074,
being done, is not edited.

## Multiple implementation steps

1. **L01** with R4's visibility change deferred — the path R11 is about.
2. **L02** — both second factors, including recovery-code and TOTP-step rollback.
3. **L03** and **L04**.
4. **R4** — `sessions::insert` crate-private; manifest and structural gate
   updated; `ci/audit-coverage-matrix.md` classes updated.
5. `docs/threat-model.md` states the rule (through RFC 097's baseline if that has
   landed first, otherwise directly), and RFC 094/096 pointers are confirmed.

Each step is independently reviewable and leaves every unconverted path as it
was.

## Test plan

For **each** command L01–L04:

- **Happy path:** one session, one event, bookkeeping applied.
- **Injected append failure** (RFC 094's failure-injection seam): no session row,
  no event, counter and lock unchanged, `last_login_at` unchanged, pending row
  still present (L02), TOTP step unchanged and recovery code still present (L02),
  no over-cap eviction; the HTTP response is byte-identical to that step's
  ordinary failure; the error log line is emitted.
- **Concurrency (L02):** two completions of one pending row — exactly one session
  and one event; one TOTP code submitted on two pending rows for the same user —
  exactly one session.
- **Revalidation:** the user disabled between verification and commit — rolled
  back, uniform failure.
- **R4:** a compile-negative or structural fixture showing no production caller of
  the raw session insert outside the commands.
- **R11 end-to-end:** with the audit log failing, wrong passwords and a correct
  password both return the uniform 401 and no session is ever created.

## Security considerations

**The attack this closes.** Induce or wait for an audit-log write failure; guess
passwords without lockout (U22 fails); sign in with the right one, unrecorded.
After this RFC the last step fails too, so the guessing gains nothing during the
outage.

**The cost is availability, stated plainly.** While the audit log cannot be
written, nobody can sign in. Audit rows and sessions share one SQLite database,
so most causes of an audit-write failure also break the session write and would
have failed sign-in anyway; the remaining cases (the audit chain's own
constraints, a full-table or trigger fault specific to `audit_log`) are exactly
the cases the attacker exploits. R3's error log makes the outage visible to the
operator instead of silent.

**What does not change.** Existing sessions keep working during an audit outage;
this RFC governs establishing a session, not using one. Step-up
re-authentication is out of scope: it creates no session, and every dangerous
action it unlocks is itself a Class-A command that cannot commit unaudited.
Failure and denial events stay Class B; a failure that cannot be recorded is
still a failure.

**Threat model.** `docs/threat-model.md` is the single home for security claims
(RFC 098 rule 7). This section is the decision; the document states the
resulting property.

## Findings recorded while scoping — not in scope

Measured by reading, 2026-09-16; each needs its own decision, not this RFC's:

- Path 4 labels an LDAP-sourced session `amr: [fed]`, although no federation took
  place.
- Paths 4 and 5 set a 24-hour session lifetime directly; paths 1–3 use
  `SESSION_LIFETIME_HOURS`, which is 12 — so external-source and federated
  sessions last twice as long, by no recorded decision.
- The settings log summary and the dashboard count only `auth.login.success`, so
  sign-ins completed through a second factor or federation are not counted as
  sign-ins.

## Open questions

1. **Type-level proof of authentication.** Should the commands require a sealed
   proof value that only the verification functions can construct, rather than
   relying on a caller list and the structural gate? Stronger, but the proof type
   would have to live in `sui-id-store` or a crate both sides depend on. Routes to
   `@nabbisen` if the design reviewer cannot adjudicate it.
2. **Rolled-back TOTP step.** Is keeping a code usable after a failed commit
   (above) acceptable, or should the step advance in a separate committed write
   even when the sign-in fails? The recommendation is the former; the latter
   burns a user's valid code on a server fault.
