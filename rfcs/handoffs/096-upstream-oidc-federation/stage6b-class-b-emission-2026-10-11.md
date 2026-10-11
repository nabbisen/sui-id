# RFC 096-B1 stage 6b — `emit_must_attempt`, the Class-B emission path

**Dispatched.** 2026-10-11 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
The piece itself is **RFC 094 `:292-298`**, also Accepted; built here for the
reason recorded in
[`emit-must-attempt-prerequisite-decision-2026-10-11.md`](emit-must-attempt-prerequisite-decision-2026-10-11.md).
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
amended and authorized 2026-10-11. Stage 6's second part.
**Prior.** Stages 0–5 and 6a complete. Stage 6a landed the attempt purge and
the `Terminal` cookie guarantee.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

## Why this stage exists at all

F05 — terminal attempt failure, 096-B1's next command — specifies *"exact
Class-B sum: `auth.federation.signin.upstream_failure`, `takeover_blocked`,
`link_required`, or `signin.denied` with closed reason enum"*. **There is no
Class-B emission path to build that on.** `AuditClass::MustAttempt` exists at
`registry.rs:118-124` carrying its own admission — *"not implemented in this
Stage-1 slice — no slice command needs it"* — `emit_must_attempt` appears in
that comment and nowhere else in `crates/`, and `commands/tests.rs:162`
asserts that no descriptor uses the class.

So this stage builds the mechanism. F05 (stage 6c) then uses it.

## Scope — the mechanism, and exactly one call site

RFC 094 `:292-298` is the specification and it is short:

> Replace silent `let _ = audit::append(...)` behavior with a named
> `emit_must_attempt` API. It returns/records an outcome and always emits
> structured error telemetry on failure. Callers may preserve the primary
> denial or detection response, but tests can inject failure and assert that
> the failure signal occurred. Raw append remains internal to the audit
> subsystem.

Four requirements, each of which needs to be visible in what you return:

1. **Named API, in the audit subsystem.** Not a helper in a handler.
2. **Returns or records an outcome** — the caller can tell that the emission
   failed, and a caller that ignores it does so visibly rather than by
   `let _ =`.
3. **Always emits structured error telemetry on failure** — a failed audit
   emission is never silent, even when the request succeeds.
4. **Failure is injectable, and a test asserts the signal fired.** This is the
   one that makes requirements 2 and 3 real rather than claimed. Without it the
   whole thing is a rename.

**The events must be registry-declared, not free strings.** The four sites this
stage touches use string literals today
(`federation_provider::AUDIT_TAKEOVER_BLOCKED` and friends). A `MustAttempt`
emission should name an `EventDescriptor` the same way a Class-A commit does,
so the audit matrix and G13 bind it. Design the declaration shape you think is
right and say why — `declare_write_command!`'s sealed shape is the obvious
model, but a Class-B event is not a command and forcing it through a command
macro may be the wrong fit. **Say which you chose and what the alternative cost
would have been.**

**One call site, not twenty-one.** There are **21** best-effort
`let _ = … audit::append(…)` sites in production across 13 files. Convert
**one**: `handlers/federation.rs`'s takeover-blocked emission, which F05 will
own anyway. That proves the mechanism end to end on a real path. **The other 20
are explicitly not yours this stage** (§"Not in scope").

## Two gates will bite, and you should know where

**`commands/tests.rs:162`, `every_descriptor_is_atomic_class`,** asserts that
every descriptor is `Atomic`. The first `MustAttempt` descriptor breaks it.
**That test is a guard and weakening it to assert nothing is not acceptable.**
Its comment says what it is really for — *"no Class-B command is in this
slice"* — so the replacement must keep the property that matters: **every
descriptor reachable from a sealed Class-A command is `Atomic`**, and a
`MustAttempt` descriptor is reachable only through the must-attempt path. If
the registry's own types can carry that distinction so the test becomes
redundant, better still — say so.

**G13's column check** (`scripts/check-audit-matrix-columns.py:197-216`) ties
`contracts/audit-coverage-matrix.md`'s `Class` column to the sealed
descriptors. Measured, so you are not guessing: a row claiming **A** must be
carried by a sealed `Atomic` descriptor; a row claiming **B** must **not** be
carried by a sealed *Atomic* descriptor — a `MustAttempt` descriptor is
compatible with a B row. But the **Actor** check at `:217-228` applies to *any*
descriptor the matrix names, so a new `MustAttempt` descriptor's
`ActorRequirement` must match its matrix row's Actor cell.

## Not in scope

**The other 20 best-effort append sites.** *"Raw append remains internal to the
audit subsystem"* can only become true when all 21 are converted and
`audit::append` is made private, and RFC 094's **M2b** already owns *"no
production Class-A best-effort append anywhere"*. Converting them here would be
096-B1 absorbing M2b. **List them in your package** — path and line — so M2b's
inventory inherits a measured list rather than a grep.

**F05 itself.** No `status = 'failed'` transition, no terminalization, no
closed reason enum beyond what the one converted site needs. That is stage 6c.

**F01.** Its class is still open — see §4 of the prerequisite decision — and it
is not this stage's business either way.

**`audit::append`'s visibility.** Leave it `pub`. Making it private is the
twenty-first site's problem, not the first's.

## Carried in from stage 6a's review: the removal cookies have no `Path`

Not a defect today, and it was traced before being written down. Both
federation cookies are **set** with `Path=/auth/federated`
(`handlers/federation.rs:413`, `:445`). `terminal()` removes them with
`Cookie::build(STATE_COOKIE)` and `Cookie::build(BROWSER_BINDING_COOKIE)` —
**no `Path`**. A removal `Set-Cookie` without one takes the browser's
default-path, the request URI up to its last `/`; every `Terminal` today is
emitted from `/auth/federated/callback`, whose default-path is
`/auth/federated`, so it matches and the removal works.
`complete_federated_signin` has exactly one caller, inside
`federated_callback`, so there is no second URI in play.

So the guarantee stage 6a established rests on a coincidence between the
endpoint's path and the cookie's. Build the removal cookies with
`.path("/auth/federated")` — the same literal the setters use — and make
`assert_cookie_expired` in `tests/e2e/r096_b1_stage6a.rs` assert
`Path=/auth/federated` as well as `Max-Age=0`, since it currently checks only
the latter and would pass either way. **Show that the strengthened assertion
fails without the `.path(...)` call**; otherwise it is two lines that prove
nothing.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The API**, with the declaration shape you chose for a Class-B event and
   the one you rejected.
2. **The outcome type**, and what a caller that ignores it looks like — show
   that ignoring it is visible in the source rather than idiomatic.
3. **The telemetry on failure**, and the test that injects a failure and
   asserts the signal fired. Name the injection mechanism; if the audit
   subsystem has no seam for it today, building that seam is part of this
   stage, not a reason to skip the test.
4. **The one converted site**, and proof the denial still happens when the
   emission fails — A6 (`RFC 094 :120`): *"failure to log a denial/detection
   never"* prevents the denial. A test that injects the failure and asserts the
   caller still returns its redirect is the point of the whole class.
5. **The replacement for `every_descriptor_is_atomic_class`**, with the
   property it now guards stated plainly, and a demonstration that it still
   fails if a Class-A command's descriptor were made `MustAttempt`.
6. **The audit-matrix row** for the converted event, and G13 green.
7. **The list of the other 20 sites**, path and line, for M2b.
7b. **The carried cookie-`Path` item**, with the strengthened assertion and the
    demonstration that it fails without the fix.
8. **Mutation evidence**: break the telemetry-on-failure path; break the
   outcome so a failure reports success; make one Class-A descriptor
   `MustAttempt`. Each should fail exactly one thing, and say which.
9. **Per-hunk SHA-256** against the tip you named, full-content hashes for new
   files.
10. **Gate evidence** from a throwaway clone with its own `target/`, plus
    `cargo test --workspace` on both pinned toolchains. **G13 and G17 are both
    owed** — G13 because the matrix moves, G17 because if you add a manifest
    row this stage you must say which and why, and if you add none, confirm
    that against the index.
11. **Anything you think is wrong with this dispatch.** The counts here are
    mine and checkable: **21** best-effort sites in **13** files, **4** of them
    in `handlers/federation.rs`. *(I started to quote the matrix's A/B row
    census too and removed it: my own count disagreed with itself across two
    parses, and a number I cannot reproduce exactly does not belong in a
    dispatch. The rule above is what you need; count the rows yourself if you
    need the census.)* The design of the declaration shape is deliberately left
    to you, so if the
    model I gestured at is the wrong fit, saying so is the answer rather than a
    blocker.
