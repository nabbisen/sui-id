# RFC 096-B1 stage 11 — the third method, end to end

**Dispatched.** 2026-10-15 JST, by the architect.
**RFC.** 096 — **Status: Accepted**, re-accepted 2026-10-14.
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md).
**Prior.** Stage 10b landed `f3e6925`, Level B, CI `38665382301` — 27 jobs, 0
skipped.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.
**Release.** Cycle D is gated on stage 12. Nothing here is rushed by a release.

## 0 — the scope, and why it is one stage rather than two

The stage plan's own line for stage 11 reads "F06. The federated WebAuthn
ceremony" — narrower than what the stage 10a dispatch assumed ("stage 11 keeps
F06 and the WebAuthn branch"). That drift is mine; I am resolving it toward the
wider reading and correcting the plan in the same change.

**F06 alone would write a ceremony nothing reads.** That is the shape this
project has just spent a release decision refusing: stages 8, 9 and 10a are
sitting unreleased precisely because they are inert, and cycle D waits on stage
12 to make them real. Adding a fourth inert piece is the wrong trade when the
alternative is available.

And unlike stage 10, there is **no open architectural question to settle
first**. The placement is decided: the verifier and the candidates are
`sui-id-core`'s, F03 is `sui-id-store`'s, the promotion logic sits in core
beside the verifier, and the seam is held by a source-scope test. Stage 10's
split bought a decision; here it would buy nothing.

So stage 11 is **F06, the `ValidWebAuthn` candidate, the verifier's WebAuthn
path, and F03's WebAuthn branch** — the third method working end to end, with
no handler. If, having measured it, you think the seam between "ceremony
exists and verifies" and "F03 consumes it" is worth a split after all, say so
with what you measured; that argument carried stage 10 and it would carry here.

## 1 — what already exists, measured

- **The table.** `federation_webauthn_ceremony`, migration `0047:183`. Its
  shape does three of F06's jobs for you: `pending_id TEXT NOT NULL UNIQUE`
  **is** the at-most-one-ceremony-per-continuation rule; `expires_at`'s own
  `CHECK` enforces `expires_at <= pending_expires_at`; and the composite
  `FOREIGN KEY` is what makes the copied `pending_expires_at` truthful rather
  than merely conventional — a row whose copy is stale has no matching parent.
  Read the column comments before designing around them.
- **The local WebAuthn surface to mirror.**
  `crates/sui-id-core/src/authn/webauthn.rs` (355 lines) — `build`,
  `start_authentication`, `finish_authentication`, `has_credentials`. The
  federated path is the same ceremony with a different home for its state.
- **`webauthn-rs`** is a `sui-id-core` dependency, not `sui-id-store`'s. That
  is settled and is part of why the verifier lives where it does.
- **No repo module** for the ceremony table yet. You write it.

## 2 — F06

`contracts/write-commands.toml`'s F06 row states the mutation surface:

> require pending F02 row count 0–4 and exact unexpired
> provider/version/generation/link/user; atomically replace at most one
> `FederatedLogin` ceremony bound to parent/RP/origin/challenge and capped
> expiry

Class P, so it is a `db.protocol` transaction and it **grants no authentication
authority by itself** (`:822-824`). Its guards are F03's guards minus the proof
— reuse the shape, do not re-derive it.

**`sealed = false` on that row when you flip it to `implemented`.**
`check-write-commands.py:248` enforces "a sealed command is Class A by
construction" and F06 is Class P. I got this wrong for F03 last round and you
were right to override me; do not let my earlier wording mislead you again.

## 3 — the candidate and the verifier's WebAuthn path

`ValidWebAuthn` is the fourth member of the family stage 10a built, and it gets
the same treatment: private fields, no public constructor, no `Clone`, `Debug`
or `Serialize`, and **a `trybuild` fixture proving at least the construction
half** in `crates/sui-id/tests/compile_fail/`. Stage 10a left the variant out
deliberately — "stage 11 adds the variant when it adds the ceremony read that
could produce one" — and this is that stage.

The verifier's WebAuthn entry point mirrors the local
`finish_authentication`, reading `ceremony_state_sealed` rather than a session.
`:794-796`: the ceremony is bound to pending ID,
provider/version/activation generation/link/user, RP/origin/challenge, and an
expiry no later than the pending row. Check what the row binds; the table
enforces some of it and the code must enforce the rest.

**`MethodMismatch` becomes producible here.** `:801`: "TOTP/recovery proof
cannot enter the WebAuthn branch or vice versa." Stage 10a left that reason
unreachable; this stage makes it reachable, and it earns a test in both
directions.

## 4 — F03's WebAuthn branch

`:796-800`, quoted:

> F03 requires that exact ceremony and credential, revalidates the sealed
> WebAuthn result, then consumes ceremony/pending and updates the passkey
> counter with the session. Wrong WebAuthn completion increments the shared
> failure count and consumes only that failed ceremony; retry requires F06.

Three things in that worth stating back:

- **"the shared failure count"** — the same `failure_count` on the pending row
  that TOTP and recovery increment. Not a second counter.
- **"consumes only that failed ceremony"** — a wrong WebAuthn leaves the
  pending row alive (unless the increment hits 5) but destroys the ceremony, so
  a retry must call F06 again. That asymmetry against TOTP/recovery is
  deliberate and needs its own test.
- **"updates the passkey counter with the session"** — the counter write is
  inside the promotion transaction, like the TOTP step and the recovery blob.

`CeremonyInvalid` becomes producible: it is the reason for a ceremony that is
missing, expired, or bound to something other than what was presented.

## 5 — the scope test extends

`crates/sui-id/tests/e2e/r096_b1_stage10b_f03_scope.rs` asserts nothing in
`crates/sui-id/src/` names `complete_federated_mfa`. F06 is a `pub` store
command with the same property — only the verifier's own path may call it — so
extend the test to cover it rather than writing a second one, and say in the
package that you ran the mutation for the new name too.

## 6 — events

No new event names. `auth.federation.mfa_rejected`'s `webauthn` and
`method_mismatch` reasons become producible, and `auth.federation.mfa_success`
gains `method=webauthn`. Check whether the matrix row's attribute column needs
widening for that and say what you found.

## What this stage must not do

**No handler.** Stage 12 wires `/admin/login/mfa`. The verifier and F03 still
have no caller from `sui-id` after this stage; say so plainly.

**No new migration.** `0047` already has the table. If you believe it needs a
column, that is a finding, not a change.

**No `UNIQUE (user_id)`** on `federation_mfa_pending`. That question stays open
with the F02/F03 pair.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **F06**, its guards, and how the table's own `UNIQUE`/`CHECK`/composite-FK
   do or do not cover each one — say which invariant is the schema's and which
   is the code's.
2. **The candidate and its compile-fail fixture.**
3. **The verifier's WebAuthn path**, with `MethodMismatch` tested both ways.
4. **F03's WebAuthn branch**, with the consumes-only-the-ceremony asymmetry
   tested, and the shared counter shown to be shared.
5. **The extended scope test**, with a mutation for the new name.
6. **Per-hunk SHA-256** against the tip you named.
7. **The full matrix, all 24 green.** Not 23.
8. **Count reconciliation** from `f3e6925`: `sui-id-core --lib` **190**,
   `sui-id-store --lib` **476**, `sui-id --lib` **560**, `sui-id --test e2e`
   **533**, `sui-id --test compile_fail` **11**, `sui-id-i18n` **19**,
   `scripts/tests` **349**.
9. **Anything wrong with this dispatch**, including §0's decision not to split.
   Over the last four rounds you have corrected me on the trybuild harness, the
   `sealed` flag, the G15 framing, an RFC citation, a factual error in an RFC I
   wrote, and a consequence I asserted without reading the schema's own foreign
   key. That rate is the useful number.
