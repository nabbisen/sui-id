# RFC 096-B1 stage 8 — migration `0047`: `federation_mfa_pending` and `federation_webauthn_ceremony`

**Dispatched.** 2026-10-13 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
its stage 8.
**Prior.** Stages 0–5, 6a–6c, 6e and 7 complete. Stage 7 landed `e9d5001`.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

**Schema only**, the same way stage 1 was: these tables get created here and
written for the first time in stage 9 (F02). Migration evidence is a `:23`
closure item, so it is produced here rather than reconstructed later.

## Scope — RFC 096 `:728-738`, quoted whole

> The migration creates a dedicated `federation_mfa_pending` row rather than
> weakening the current generic user-only pending-MFA shape. It stores
> provider/version/activation generation/link/user, sealed continuation/next
> target, browser and CSRF binding digests, `primary_method=Fed`,
> `failure_count` constrained 0–5, `status` in
> `pending|completed|exhausted|invalidated`, `created_at`, and
> `expires_at=created_at+300 seconds`. Exactly `expires_at <= guard_at` is
> expired. Terminal rows retain no usable continuation and are purged after 24
> hours. A restart preserves count/status; it grants no reset. A separate
> `federation_webauthn_ceremony` table has a unique parent-pending key, exact
> provider/version/activation generation/link/user binding, sealed ceremony
> state, RP/origin, and no-later expiry; generic password/WebAuthn pending
> rows cannot reference it.

Every clause in that paragraph is a schema obligation or a constraint. Work
through them one at a time and say, per clause, whether the schema enforces it
or a later stage's code must — **and if it is the code's, say which stage**.
That division is the thing I will read most carefully, because "the code will
check it" is how a constraint becomes nobody's.

### What `0046` already settled, and should be followed

Read `0046_federation_login_attempt.sql` before writing a line. It is the
nearest model and its own comments argue the choices:

- **`STRICT`**, which is what stops a 64-char hex string passing as a 32-byte
  `BLOB` where a `length(...) = 32` check alone would admit it.
- **Digests as `BLOB NOT NULL CHECK (length(...) = 32)`**, not hex text.
- **A table-level `CHECK` making two columns agree** rather than trusting
  every future writer — `0046` does this for `claimed_at` against `status`.
  `:734`'s *"terminal rows retain no usable continuation"* is the same shape
  and wants the same treatment: a row whose `status` is terminal and whose
  continuation is still present should not be representable.
- **A reasoned decision about `FOREIGN KEY`**, stated in a comment either way.
  `0046` has none on `provider_id` and explains why at length; `federation_link`
  cascades to the same table. **These two tables are not obviously the same
  case as either** — a pending MFA row belongs to a live user and a live link,
  which argues for cascade; `federation_webauthn_ceremony`'s parent key argues
  for cascade to `federation_mfa_pending`. Decide, and write the reason into
  the migration.

### The clauses that need thought, not transcription

1. **`failure_count` constrained 0–5.** The constraint is the schema's. The
   *semantics* — 1–4 is `RejectedStillPending`, exactly 5 is `AttemptsExhausted`,
   no sixth increment exists (`:741-751`) — are stage 10's. A `CHECK` that
   admits 0–5 and nothing else is what this stage owes.
2. **`expires_at = created_at + 300 seconds`.** Can a `CHECK` express it?
   SQLite can compare two `TEXT` timestamps, but the arithmetic depends on the
   storage format. Say what you concluded and why; if the schema cannot
   enforce it, say which stage's code must and make that explicit rather than
   implied.
3. **`primary_method = Fed`.** A one-valued column is either a `CHECK`
   pinning it or a column that should not exist. Argue which, and if you keep
   it, say what a future second value would mean.
4. **`:738`'s *"generic password/WebAuthn pending rows cannot reference it"*.**
   This is a negative requirement about two *other* tables
   (`login_pending_mfa`, `webauthn_pending`, migrations 0003/0004). Read them.
   Say how the schema makes the cross-reference impossible rather than merely
   absent — and if the honest answer is that nothing structurally prevents a
   future writer from putting a `federation_mfa_pending` id into a generic
   row's column, say that plainly and name what would.
5. **No-later expiry** for the ceremony than its parent. A `CHECK` cannot see
   another table's row. Decide where it lives and say so.

## Not in scope

**Any writer.** No insert, no claim, no promotion. Stage 9 writes the first
row; stages 10 and 11 consume them.

**The purge.** `:734` requires terminal rows to be purged after 24 hours, and
stage 6a's lesson is that an unpurged table is a growth leak from its first
writer onward. **It is not this stage's** — there is nothing to purge yet —
**but it is stage 9's, with the first writer**, and I am recording that here
so it does not become nobody's. Say in your package that you have read this
and that stage 9 owes it.

**`FederatedMfaVerifier`, F02, F03, F06.** Stages 9–11.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The migration**, with a comment-for-comment account of the five items
   above and the `FOREIGN KEY` decision.
2. **The clause table**: every obligation in `:728-738`, and for each, whether
   the schema enforces it or which stage's code will.
3. **Negative fixtures.** The `0046` round's own standard: a row that violates
   each `CHECK` is refused, demonstrated, not asserted. In particular
   `failure_count = 6`, `failure_count = -1`, an unknown `status`, a terminal
   row that still carries a continuation, and — if you make it enforceable —
   a ceremony outliving its parent.
4. **The `STRICT` demonstration**: text where a `BLOB` is expected is refused.
5. **Migration evidence** proper, since it is a `:23` closure item: the
   migration applies to a fresh database and to one at `0046`, and the result
   is identical. Say how you checked "identical".
6. **Per-hunk SHA-256** against the tip you named, full-content hashes for new
   files.
7. **Gate evidence**: the full matrix from a throwaway clone with its own
   `target/`, plus `cargo test --workspace` on both pinned toolchains. **G19
   is the one to watch** — migrations are its subject — and say what it checks
   rather than only that it passed.
8. **The count reconciliation** from `sui-id-store --lib` **406**,
   `sui-id --lib` **560**, `sui-id --test e2e` **532**, `sui-id-i18n` **19**,
   `scripts/tests` **349**. *(The e2e figure is the one my last dispatch got
   wrong by carrying a stale number; these are measured on `e9d5001`.)*
9. **Anything you think is wrong with this dispatch.** Item 4 of §"clauses" is
   the one I am least sure of: I have not read `login_pending_mfa` and
   `webauthn_pending` closely enough to know whether `:738`'s requirement is
   already satisfied by their shape, newly satisfiable, or not satisfiable in
   the schema at all. If it is the third, that is a finding and not a failure.
