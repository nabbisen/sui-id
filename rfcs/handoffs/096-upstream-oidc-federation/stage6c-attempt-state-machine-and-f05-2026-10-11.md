# RFC 096-B1 stage 6c — the attempt state machine: claim before exchange, browser binding, and F05

**Dispatched.** 2026-10-11 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
amended and authorized 2026-10-11. Stage 6's third part.
**Prior.** Stages 0–5, 6a and 6b complete. 6b landed `emit_must_attempt`, which
this stage's events are emitted through.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

## This stage is bigger than "F05", and the reason is my error

The plan's stage 6c was *"F05 — terminal attempt failure on that mechanism"*.
Writing it, I checked the callback's actual order of operations against RFC 096
`:597-599` and found that **two things the RFC requires of the claim are not
what the code does**, both because of how I dispatched stages 3 and 4. F05
cannot be wired correctly on top of them, so they come first, in this stage.

RFC 096 `:597-599`, quoted whole because every clause in it matters:

> For a code response, one immediate database transaction matches state digest,
> provider ID/version/generation, pending state, unexpired time, and
> **constant-time browser binding**; it changes `pending` to `exchanging`
> **before token exchange**. Exactly one affected row is required.

Measured against `handlers/federation.rs` as it stands:

| Step | Line | RFC says |
|---|---|---|
| upstream `error` response → return | `:507` | `:603-604`: *"Error responses consume the matching attempt without token exchange"* — nothing is consumed |
| state cookie unsealed, `ct_eq` against `q.state` | `:523-537` | — |
| provider loaded, `enabled` checked | `:550` | — |
| discovery fetched | `:558` | — |
| **token exchange POSTed** | `:622` | **after** the claim, not before |
| `verify_id_token` | `:713` | — |
| **attempt found** by `state_sha256` | `:736` | the lookup has to precede everything that can fail |
| **claim** `pending → exchanging`, bundled with nonce consumption | `:752` | the claim belongs **before** the exchange; the nonce comparison cannot, because it needs the ID token |
| browser binding compared | **nowhere** | `:598` requires it, in constant time, as part of the claim |

**Three divergences, all mine.** Stage 3's dispatch told you to claim the
attempt *and* consume the nonce in one operation, which forces the claim to
wait for the ID token. Stage 4's told you to route the callback through the
validators without saying where the claim sat relative to the exchange. And
neither mentioned `:598`'s browser binding, which is why
`browser_binding_sha256` has been stored since stage 1 and compared against
nothing since — it has been on my standing limitations list for three stages
as a thing to fix "later", and `:597-599` says it is part of the claim.

**Why they cannot wait for a later stage.** Moving the claim earlier widens the
window in which a failure leaves a row `exchanging`, so terminalization has to
land with it, not after it. And F01 (stage 6d) builds on the
`exchanging → completed` transition, so it needs the state machine to be right
first. Doing the three together is the smallest correct unit.

## Scope

### 1. Split the claim from the nonce comparison (`:597-599`, `:65-68`)

`claim_and_consume_nonce` bundles two things the RFC separates. Split them:

- **The claim** — `pending → exchanging`, one transaction, exactly one affected
  row — moves to **immediately after the state check and provider load**, and
  **before** `fetch_discovery` and the token exchange. It needs no ID token.
- **The nonce comparison** — the stored `nonce_sha256` against the ID token's
  `nonce` — stays where the ID token exists, after `verify_id_token`.

`:65-68` is the clause that separates them in the first place: 096-A delivers
*"the nonce validation rule"*, 096-B1 *"the durable attempt state that makes a
nonce genuinely one-time"*. They are two mechanisms and the code has them as
one call.

**What the claim transaction must match**, per `:597-598`, all of it in the one
transaction: state digest, provider ID **and version and generation**, pending
state, unexpired time, and the browser binding. It already has state digest (by
`find_by_state_sha256` then claim by id), pending state and unexpired time, and
it has clock-regression refusal, which keep. Add the rest.

**Note what the version/generation match means today** and do not overstate it:
`placeholder_provider_version_and_generation` returns `(0, 0)` until RFC 094 M2b
adds the columns, so comparing the row's values against "current" is
tautological — stage 4's review established that and the function is named for
it. Wire the comparison into the claim because `:597` puts it there; say
plainly in the code and in your package that it cannot disagree yet.

### 2. The browser binding, in constant time (`:598`)

`BROWSER_BINDING_COOKIE` is set at `:420` and read nowhere. The attempt row
holds `browser_binding_sha256`. Compare them as part of the claim:

- Read the cookie, SHA-256 it, compare against the stored digest **in constant
  time** — `subtle::ConstantTimeEq`, as the `state` check at `:530-536` already
  does. Note that `ct_eq` on slices does not hide length, which is fine here
  because both sides are fixed-size 32-byte digests; say so rather than leave a
  reader to wonder.
- **A missing cookie is a failure, not a bypass.** Decide and state what a
  request with no binding cookie gets, and make it the same outcome shape as a
  mismatch.
- This closes a standing limitation that has been on the closure list since
  stage 4. Say in your package that it is closed, so the closure assessment can
  drop it.

### 3. F05 — terminal attempt failure (`:601-605`)

> No attempt returns to pending: transport, token, validation, disabled
> provider, or mapping failure changes exchanging to failed or leaves it safely
> reconcilable as failed; the browser must start again. Error responses consume
> the matching attempt without token exchange.

- **The command**: guarded `pending|exchanging → failed`, one-time, exactly one
  affected row required. **No link, user, session or last-seen mutation** —
  that is in F05's own manifest row and it is the property to prove.
- **Its events**, through 6b's `emit_must_attempt`: the manifest names an
  *"exact Class-B sum"* — `auth.federation.signin.upstream_failure`,
  `takeover_blocked`, `link_required`, or `signin.denied` **with a closed
  reason enum**. Closed means the handler cannot invent a reason string; the
  enum is the only way to name one.
- **Every failure path after the claim terminalizes.** After item 1 that is
  most of the callback. Enumerate them in your package with line numbers and
  say which event and reason each maps to — if two paths map to the same
  reason, say why that is right rather than collapsing them silently.
- **`:603-604`'s error-response path.** The upstream `error` response at `:507`
  must consume the matching attempt *without* a token exchange. It returns
  before the state cookie is even unsealed today. The attempt is findable from
  the `state` the error response carries; work out the right order and say what
  you chose, including what happens when the state is absent or matches no row
  (an unauthenticated caller can send anything — this must not become a way to
  probe which state digests exist).
- **`takeover_blocked` already exists** as a 6b descriptor and is emitted from
  the P2 collision block. Decide whether F05 subsumes that emission or sits
  beside it, and say which. Do **not** change the row's `(A required)` marking
  in `contracts/audit-coverage-matrix.md` — that is `@nabbisen`'s to change and
  a request is open with him.
- **New matrix rows are owed** for any event name that does not yet appear:
  `auth.federation.signin.upstream_failure` and `auth.federation.link.created`
  are already there, `link_required` and `signin.denied` are not. G13's
  backward direction will demand them.

### 4. Carried from stage 6b's review: a descriptor the parser cannot see fails open

Both descriptor patterns in `scripts/rust_registry.py` need an unqualified type,
on one line, with an inlined literal name. A declaration that drifts from that
shape is dropped silently and its matrix row goes back to being checked for
nothing — which is how `AUDIT_TAKEOVER_BLOCKED_DESCRIPTOR` was invisible even
after the parser was fixed. **This stage adds four more `MustAttemptDescriptor`s**,
so it is the right place for the guard: count the `MustAttemptDescriptor {` and
`EventDescriptor {` static declarations in source and fail if the parser's own
tally is lower. Put it where a gate runs it, add a fixture that a drifted
declaration fails, and report both tallies.

## Not in scope

**F01.** Stage 6d, and still gated on the RFC 094 record question.

**The legacy identity-mapping path.** Stage 12 removes it. This stage makes the
attempt state machine correct; it does not change which code decides the
outcome.

**`federated_start`'s own early-failure cookie hygiene** (S1) and the
`r120_routes.rs` visibility matcher (S2). Both scheduled elsewhere.

**The `(A required)` marking.** Open with the owner. Leave it exactly as it is.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The split**, with the claim's new position named by line and the reason the
   nonce comparison cannot move with it.
2. **The claim's full match set**, per `:597-598`, with the version/generation
   tautology stated rather than implied.
3. **The browser binding**, its constant-time comparison, its missing-cookie
   behaviour, and a test for each of: match, mismatch, absent.
4. **F05**, its closed reason enum, and the **enumerated** failure-path table
   with line numbers and the event/reason each maps to.
5. **The error-response path**, with your chosen order and what an absent or
   unmatched state does.
6. **Proof of the no-side-effect property**: F05 touches no link, user, session
   or last-seen row. A test that asserts the absence, not an inspection.
7. **A test that a terminalized attempt cannot be terminalized twice**, and one
   that a `completed` attempt cannot be moved to `failed`.
8. **The item-4 guard**, with both tallies and the drifted-declaration fixture.
9. **Mutation evidence**: claim moved back after the exchange; the binding
   comparison short-circuited; one failure path's terminalization removed; the
   reason enum's closedness broken. Each should fail exactly one thing.
10. **Per-hunk SHA-256** against the tip you named, full-content hashes for new
    files.
11. **Gate evidence**: the full matrix from a throwaway clone with its own
    `target/`, `cargo test --workspace` on both pinned toolchains, **G13 and
    G17 both owed** (the matrix moves; F05 gets a manifest row — it is
    `status = "target-legacy", sealed = false` today and this stage implements
    it, so say what you changed it to and why), and **A3.2** if you touch
    `scripts/`.
12. **The count reconciliation**, starting from `sui-id-store` lib **382** and
    `sui-id` e2e **514**, which are this baseline's measured numbers.
13. **Anything you think is wrong with this dispatch.** The order table at the
    top is mine and every line number in it is checkable. The claim-before-
    exchange reading of `:597-599` is the load-bearing one: if you think the
    sentence can be read so that today's order satisfies it, say so — I would
    rather argue about it now than reorder a callback on a misreading.
