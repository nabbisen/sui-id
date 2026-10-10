# RFC 096-B1 stage 2 — durable attempt start

**Dispatched.** 2026-10-10 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
authorized 2026-10-09 — **with two corrections to its command mapping, below.**
**Prior.** Stage 0 (`ReadConn`) and stage 1 (migration `0046`) complete.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

## Two corrections to the plan, both mine, found before dispatching

**1. The plan's F-command assignments are wrong.** It said stage 2 implements
"F01, F02". It does not. Read from `contracts/write-commands.toml`:

| | `mutation_surface`, abbreviated | What it actually is |
|---|---|---|
| F01 | guarded attempt `exchanging -> completed`; insert session `[Fed]`; session cap | existing-link login — **stage 6** |
| F02 | guarded attempt `exchanging -> completed`; insert pending MFA row | existing-link login needing MFA — **stage 6** |
| F03 | guard pending MFA row, proof counting 0–4 | **federated MFA submission**, not nonce consumption |
| F04 | guard attempt `exchanging -> completed`; recheck absent link + verified unique email | first provisioning + login, Class **A** — **stage 6** |
| F05 | guarded attempt `pending/exchanging -> failed` | **terminal failure**, not identity mapping |
| F06 | replace a `FederatedLogin` WebAuthn ceremony | **WebAuthn MFA ceremony** |

So four of the plan's five F-assignments were wrong — only F04-as-Class-A was
right. **The stage *order* is unaffected** and stays as authorized; what was
wrong is which registered command each stage implements, and the corrected
mapping is going into the plan document alongside this dispatch.

**2. Nothing in the manifest creates an attempt row.** All 101 registered
commands, searched: the only three mentions of `federation_login_attempt` are
`exchanging -> completed` guards. **Neither the `pending` insert (this stage) nor
the `pending -> exchanging` claim (stage 3) is a registered command.**

That is this stage's first problem to solve, not a footnote.

## The registration question — decide and argue it

G17 (`scripts/check-write-commands.py`) enforces, direction (A): **every command
declared with `declare_write_command!` in production code under `crates/` must
have a manifest row saying `sealed = true`.** F01–F06 are all `sealed = false`
planned rows naming no command.

So if attempt start is a `declare_write_command!`, **it needs a manifest row that
does not yet exist**, and G17 fails without one. Decide and state:

- **Is attempt start a registered write command at all?** It inserts a durable
  row, which is what the registry governs. I believe it must be, but the
  alternative — that it is deliberately outside the registry — needs refuting
  rather than ignoring, and RFC 094 is where the answer lives.
- **Its id.** F01–F06 are taken by the six RFC 094 already names. Adding F07
  asserts it belongs to that family; a different scheme says it does not. Either
  may be right; say which and why, and whether RFC 094's own numbering is yours
  to extend. **If extending it is the owner's call rather than ours, stop and
  say so** — I would rather this stage pause than quietly annex a numbering
  space.
- **Its class.** F01/F02/F03/F05/F06 are Class **P** (protocol, no audit chain);
  F04 alone is Class **A**. Attempt start precedes any authentication outcome,
  so there is no login event to chain — which argues P. Argue it rather than
  taking that.

## What the stage builds

RFC 096 `:583-589`, and only the start:

- **The `pending` row**, every column stage 1 created, with `status` defaulting
  to `pending` and `claimed_at` left `NULL` — the table-level `CHECK` requires
  exactly that pairing.
- **The sealed PKCE verifier.** `sui_id_store::crypto::seal(key, plaintext, aad)`
  exists (`crypto.rs:88`) and is XChaCha20-Poly1305 with AAD, which is what
  `:586-587` asks for. The AAD binds **attempt id, provider id, config version
  and activation generation** — all four already columns on the row, so
  reconstruct the AAD from the row rather than storing it twice. Stage 1's
  migration comment already commits to this; keep it true.
- **The 600-second lifetime** (`:588-589`): `expires_at <= now` is expired.
- **Wall-clock access injected, and clock regression fails closed** (`:589`).
  Not a detail to leave to stage 3 — 096-A stage 6b found what an in-function
  clock read does to a boundary test, and this stage sets both timestamps.

## What this stage must not do

**Not the claim.** `pending -> exchanging` is stage 3. Leave the row in
`pending`.

**And do not rely on the schema to keep the claim one-way.** Stage 1's review
established, by measurement, that `UPDATE ... SET status='pending',
claimed_at=NULL` on a claimed row **succeeds**: the table `CHECK` enforces that
`status` and `claimed_at` *agree*, not that the transition is forward-only — a
row `CHECK` cannot see where a row came from. Single-use is a security property
(`:590-592`), so stage 3 will enforce it with a conditional
`UPDATE ... WHERE status='pending'`. **Stage 2's job is not to make that harder**:
do not add a read-then-write shape, or a cached attempt handle, that stage 3
would have to undo.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The registration decision**, argued — registered or not, its id, its class,
   and whether extending RFC 094's numbering is ours to do.
2. **The row insert**, with the AAD reconstructed from the row and a test that
   opening the verifier fails if any of the four bound values is altered. That
   test is the point of the AAD; without it the binding is decorative.
3. **The lifetime and clock behaviour**, including a clock-regression test that
   fails closed.
4. **Mutation evidence** per rule, to the standard the last four packages set.
5. **Per-hunk SHA-256** against the tip you named. **Stage 1's package omitted
   this** — the first in the series to — and I computed them myself for the
   commit record. Please resume including them.
6. **Gate evidence** from a throwaway clone with its own `target/`, plus
   `cargo test --workspace`. **G17 specifically**, since this stage is the first
   to touch the command manifest.
7. **Anything you think is wrong with this dispatch.** The corrections above are
   mine, found by reading the manifest before writing the dispatch; the
   registration questions are genuinely open and I would rather have your
   reading than my guess.
