# RFC 096-B1 stage 10a — the `FederatedMfaVerifier`, its binding gate and its candidates

**Dispatched.** 2026-10-14 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
its stage 10 — **split into 10a and 10b by this dispatch**; see §0.
**Prior.** Stages 0–5, 6a–6c, 6e, 7, 8 and 9 complete. Stage 9 landed
`f5b4035` (Level B, CI run `38491673983`, 27 jobs, 0 skipped).
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

This stage builds the gate that stands in front of every federated
second-factor submission, and **writes nothing at all**. RFC 096 `:752-762`:

> Proof verification is owned by a private `FederatedMfaVerifier`, whose only
> entry point first validates the browser cookie, CSRF token, pending ID,
> user, provider/version/generation, method, and pending-row binding. Binding
> failure returns only an unbound generic error and **cannot construct an F03
> input or touch the row.** After binding, the verifier consumes bounded raw
> input and returns a sealed, non-cloneable, non-debuggable,
> non-serializable method-specific `ValidTotp`, `ValidRecovery`,
> `ValidWebAuthn`, or `BoundRejected` candidate. **Only the verifier selects
> the closed rejection reason; handlers can neither construct a candidate nor
> supply a row-touching reason.** Raw proofs are zeroized/dropped before the
> candidate crosses into F03.

## 0 — why stage 10 is split, and what 10b gets

Stage 10 as planned was the verifier **and** F03. That is a sealed type
family with a non-constructibility guarantee, plus a four-outcome guarded
transaction with three methods' anti-replay, a session, cap eviction and
three new Class-B events with closed reason enums — and a third of it
(WebAuthn) cannot be finished at all until F06 lands in stage 11. Stages 8
and 9 each ran ~10 tests and one fix round; this would be three or four times
that in one package, and the architectural question in §1 has to be settled
before any of it is written.

So: **10a is this dispatch** — the verifier, its binding gate, its candidate
family, TOTP and recovery proof checking, no writes. **10b is F03** — the
four guarded Protocol outcomes, the failure-count state machine at `:740-751`,
the `[Fed, local_method]` session and cap, and the three Class-B events at
`:797-802`. **Stage 11 keeps F06 and the WebAuthn branch**, unchanged.

This split is **the architect's scheduling proposal**, recorded as such in the
stage plan and awaiting authorization there. The precedent is stage 6's own
split into 6a–6e. If you think the boundary is in the wrong place, say so.

## 1 — where the verifier can live, which the crate graph decides

**This is the first thing to settle, before any code, and it is a real
constraint rather than a style question.** Measured, not inferred:

- `sui-id-core/Cargo.toml` depends on `sui-id-store`. `sui-id-store/Cargo.toml`
  depends on **`sui-id-shared` only**. The edge runs one way.
- `webauthn-rs` is a dependency of `sui-id-core`, not of `sui-id-store`.
- TOTP verification is `sui-id-core/src/authn/totp.rs:50-55`
  (`verify(secret: &[u8], unix_time: i64, supplied: u32, last_used_step: i64)
  -> Option<i64>`), called from `sui-id-core/src/authn/mfa.rs:113`.
- F03 is Class P, so it must live in `sui-id-store`: `WriteTx::tx()` is
  `pub(crate)` (`registry.rs:105`), which is why F02 had to. Stage 9's own
  package re-established this.

Put those together and the verifier **cannot** sit beside F03 in
`sui-id-store` while calling `sui-id-core`'s TOTP code, and a candidate type
whose constructor is `pub(crate)` to `sui-id-store` cannot be built by a
verifier living in `sui-id-core`. Rust has no "public to exactly one other
crate" visibility, so *something* has to give. The candidates I see, none of
which I am choosing for you:

1. **Move the proof primitives down.** TOTP verification is pure computation
   over a secret and a step; recovery-code matching is a hash comparison.
   Either could live in `sui-id-shared` or `sui-id-store`, putting the
   verifier beside F03 and making `pub(crate)` the whole enforcement. WebAuthn
   is the one that does not move cheaply — and it is stage 11's.
2. **Add `webauthn-rs` to `sui-id-store`** and move the verifier down whole.
   That is a new dependency in the crate that owns the database, which is an
   owner-visible trade; if you argue for it, argue the dependency too.
3. **Keep the verifier in `sui-id-core`** and find an enforcement that still
   stops the `sui-id` handler crate — a sealed trait whose only implementor is
   a `sui-id-core` type, a constructor taking a witness value only
   `sui-id-core` can mint, or similar. Say precisely what it does and does not
   stop; "handlers cannot" is the RFC's requirement, and `sui-id-core` being
   able to is a different, possibly acceptable, thing.

**What is not acceptable is the shape the local path already has.**
`SecondFactorProof` (`commands.rs:2499-2511`) is L02's analogue of a
candidate, and it is a `pub enum` with public variants and public fields —
anything that can see the crate constructs one, including
`sui-id-core/src/authn/mfa.rs`, which does. For the local path that was never
claimed otherwise. RFC 096 `:758-762` asks the federated path to be stronger,
and the gap between those two sentences is the whole of this stage's type
work. Name the comparison in your package.

## 2 — the binding gate

One entry point. It validates, in whatever order you argue for:

- the browser-binding cookie — `sui_id_fed_mfa_browser_binding`, stage 9's
  own, `/`-scoped and 300 seconds, whose digest is
  `federation_mfa_pending.browser_binding_sha256`;
- the CSRF binding — `sui_id_fed_mfa_csrf`, which per
  `repos/federation_mfa_pending.rs`'s module doc must arrive **in the request
  body**, as a hidden field rendered server-side from the `HttpOnly` cookie,
  not read back out of the cookie alone. If stage 10a renders nothing, say
  what stage does and that the comparison is written to expect a body value;
- the pending id — `sui_id_fed_mfa_pending`;
- the user, the provider triple, the method, and the row's own binding.

**Comparisons are constant-time** where a secret is on either side —
`subtle::ConstantTimeEq`, as `federation_login_attempt::claim` already does
for the browser binding.

**Two properties to prove, not assert:**

- **A binding failure returns one unbound generic error** — the same value for
  a wrong cookie, a wrong CSRF token, an unknown pending id and a pending id
  belonging to someone else. An attacker must not be able to distinguish
  "no such row" from "not your row". Test it by asserting the four produce
  the *same* error value, not merely that each is an error.
- **A binding failure touches no row.** Assert `failure_count`, `status` and
  `continuation_sealed` are all unchanged after each of the four, with a
  control in the same test showing a *successful* binding is reachable on an
  untouched second identity — otherwise "unchanged" passes for a fixture that
  never had a row to change.

Also state what happens on an **expired** pending row (`expires_at <= now`,
`:732-733`'s own "Exactly `expires_at <= guard_at` is expired"): is that a
binding failure or an `Invalidated` outcome? `:740-741` puts
`Invalidated` on authority drift/expiry, and `Invalidated` is a *write*,
so if expiry is an invalidation it belongs to 10b and 10a must hand it on
rather than swallow it. Decide and say which.

## 3 — the candidate family

`ValidTotp`, `ValidRecovery`, `ValidWebAuthn`, `BoundRejected`. Required
properties, each of which should be held by the compiler where it can be:

- **non-cloneable, non-`Debug`, non-`Serialize`** — the absence of three
  derives is the mechanism; say so, and consider whether a negative test can
  state it;
- **constructible only by the verifier** — this is the one that earns a
  **compile-fail test**, in the existing harness
  (`crates/sui-id-store/tests/compile_fail.rs` and its sibling directory;
  `protocol_cannot_construct_audited.rs` is the shape). An absence proven by a
  `trybuild` case is the project's established standard for "by
  construction", and it is what makes `:758-762` reviewable in six months;
- **`BoundRejected` carries a closed reason the verifier alone selects** —
  `:798-799`'s set for F03 is
  `totp|recovery|webauthn|malformed|method_mismatch`. A handler must not be
  able to hand in a reason that reaches the row; that is the second half of
  the same sentence and may want its own compile-fail case;
- **raw proofs zeroized before the candidate crosses into F03** — `zeroize`
  is already a dependency of `sui-id-core` (`authn/mfa.rs` uses `Zeroize`).
  Say where the raw bytes die and what guarantees it: a `Drop` impl, a
  `Zeroizing<..>` wrapper, or an explicit call on every path including the
  early returns.

**`ValidWebAuthn` in this stage.** Stage 11 owns F06 and the ceremony. Either
declare the variant now with nothing able to produce it and say so plainly,
or leave it out and say stage 11 adds it. Do not half-build a WebAuthn path.

## 4 — what this stage must not do

**No writes of any kind.** No `UPDATE` to `federation_mfa_pending`, no
`failure_count` increment, no terminalization, no session. The RFC's own
sentence is that a binding failure *cannot touch the row*; the cleanest way to
be sure of that for the whole stage is for the stage to contain no write path
at all. If you find you need one, stop and say why rather than adding it.

**No F03.** `RejectedStillPending`, `AttemptsExhausted`, `Invalidated`,
`Promoted` and the three Class-B events are 10b's.

**No change to `/admin/login/mfa`.** The generic `login_pending_mfa` row is
still what a federated user actually completes MFA through, exactly as stage 9
left it; stage 12 switches it over. If 10a's verifier has no caller yet, say
so plainly and name the stage that gives it one.

**Nothing in `contracts/write-commands.toml`.** This stage declares no write
command. If you believe it should, that is a finding, not a change.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **§1's placement decision, argued**, with the crate-graph facts you
   measured yourself rather than taken from this dispatch, and the
   `SecondFactorProof` comparison stated.
2. **The binding gate**, each check as the expression that runs, and the two
   properties of §2 proven — the indistinguishable error with four inputs, and
   the untouched row with a control.
3. **The expiry decision** (§2's last paragraph), and which stage owns it.
4. **The candidate family**, with at least one compile-fail case in the
   existing harness proving a handler cannot construct a candidate, and the
   zeroization point named.
5. **Per-hunk SHA-256** against the tip you named, full-content for new files.
   This was missing in stage 9's first round; `python3.14
   scripts/hunk-hashes.py --baseline <sha>`.
6. **Gate evidence**: the full matrix from a throwaway workspace with its own
   `target/`. **G21** (inline-test placement) and **G07/G07b** are the ones a
   new type family most often moves; if you add a compile-fail case, say which
   gate runs `tests/compile_fail.rs` and what it reported.
7. **The count reconciliation** from `sui-id-store --lib` **460**, `sui-id
   --lib` **560**, `sui-id --test e2e` **532**, `sui-id-i18n` **19**,
   `scripts/tests` **349** — measured by me on `f5b4035`, the stage 9 landing
   commit.
8. **Anything you think is wrong with this dispatch**, including the 10a/10b
   split itself. Two of the six items in stage 9's fix round were my errors;
   the rate at which you find them is the useful number, not zero.
