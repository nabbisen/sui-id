# RFC 096-B2 stage 1 — the schema, and the upgrade

> ## Whose turn it is: **the implementation role's.**
> Cycle L, 096-B2 S1 —
> [`096-b2-stage-plan-2026-11-11.md`](096-b2-stage-plan-2026-11-11.md) §3.
> **This document is the dispatch.**

**Dispatched.** 2026-11-11, by the architect.
**Governing RFC.** [RFC 096](../../accepted/096-upstream-oidc-federation-validation.md),
**Accepted**, §096-B2 as amended 2026-11-11. Building is permitted.
**Baseline.** The tip of `main` when you start (`git log -1`; record it).
**Read first:** the stage plan's §1 (what exists, with `file:line`). RFC 096
`:1018-1036` (the migration rule). `architecture.md` `:236-250` and `:300-306`.
`verification.md` §7. RFC 094 `rfcs/done/094-transactional-audit-registry.md:526-531`.

## 0 — what this stage is, and is not

**This stage adds the columns, migrates every existing provider to disabled and
review-required, and makes the code read the real values.** It adds **no**
command: no C17, C18, C20 or C23, and no admin surface. Those are S2–S6.

**The consequence is deliberate and decided** (cycle L §4, authorized): after
this stage, **every provider is disabled on upgrade, and nothing can re-enable
it until S4 lands.** So this stage must land with its operator warning
(§D), and it must never ship in a release without S4 and S5.

## A — migration 0050

**Measure the next free number first.** It is `0050` at the stage plan's
baseline. Record the number you actually use (`verification.md` §7 requires it).

The `federation_provider` columns, per RFC 094 `:526-531` and `architecture.md`
`:238-250`:
- **`config_version`**, and **`activation_generation`**. Both are `INTEGER NOT NULL
  CHECK (>= 0)`, and new and migrated rows start at `0`.
  **The architect's reading of "canonical-decimal checked `u64`":** 096-B1
  already shipped the attempt, MFA-continuation and ceremony tables with these
  values as `INTEGER … CHECK (>= 0)` (`0046:33-36`, `0047:28-29`), and the provider's
  columns must compare **exactly** against them. So use the same type. Do all
  arithmetic **checked**: `i64::MAX` is the *"maximum state"* that RFC 094
  `:528` says *"fails closed"*, and must refuse, never wrap. An `INTEGER`
  column cannot hold a non-canonical decimal, so that clause is met by the
  type. **Say so in the migration's comment.**
- **`validation_status`**: `TEXT NOT NULL CHECK (validation_status IN
  ('requires_review','validated'))`. The design names only `requires_review`;
  `validated` is the state after a successful C17 enable (S4). **This stage
  writes only `requires_review`.**
- **The preflight-evidence columns**: nullable, with no standalone writer,
  written only by C17 enable (`architecture.md:248-250`). Define them from RFC
  094 `:506-512`'s binding list (version, activation generation, policy digest,
  observation time, bounded metadata/key-set fingerprints). **This stage only
  creates them, always `NULL`.**

**The upgrade rule**, RFC 096 `:1024-1029`, in one migration:
1. preserve provider IDs, sealed secrets and links;
2. set **every** provider `enabled = 0`, `validation_status = 'requires_review'`,
   and both counters to `0`;
3. guess nothing (origin, algorithm, auth method, evidence);
4. **invalidate legacy browser-state and pending flows without unlinking
   users**. That means the `federation_login_attempt`, `federation_mfa_pending` and
   `federation_webauthn_ceremony` rows that are not terminal. **Measure what
   "invalidate" means for each table** (terminal status? delete?) against
   096-B1's own state machine, and use that table's existing invalidation, not
   a new one.

**If a `CHECK` needs a table rebuild, rebuild it** (`architecture.md:243`,
*"rather than pretending SQLite checks can be retrofitted"*). Follow
`0047`/`0048`'s rebuild pattern, and the `needs_fk_disable` handling the
migration runner already has.

## B — the real values replace the placeholder

`placeholder_provider_version_and_generation` (`crates/sui-id/src/http/handlers/federation.rs:430`)
becomes two field reads on `FederationProviderRow`. **Delete the function's
`(0, 0)`**: its two call sites (`:597`, `:814`) then read the row. Add the new
fields to the model and to every `SELECT` in `repos/federation_provider.rs`.
**Grep for every reader of the row**, and read each hit.

## C — tests: `verification.md` §7, as written

**Real pre-migration fixtures**, at least:
- zero providers;
- an **enabled** legacy provider and a disabled one;
- each `provision_mode`;
- **existing links and users**, and a **sealed secret**;
- a **malformed** row;
- an attempt, an MFA continuation and a ceremony **in flight**;
- an **interrupted and a repeated** migration.

**Prove each of these:**
- every provider ends disabled and `requires_review`, with counters at `0`;
- **links and IDs survive**, and **the sealed secret still decrypts to the same
  plaintext**;
- nothing is guessed;
- **an in-flight attempt cannot complete** after the migration;
- the rerun is deterministic;
- the fresh-install schema equals the upgraded schema.

**And one test that would have caught the gap this cycle exists for:** a
provider that was `enabled = 1` before the upgrade is **not offered on the
login screen** after it (`list_enabled` returns it no more).

**Mutation:** leave one provider enabled in the migration, and the "every
provider ends disabled" test goes red. Quote it. **G22** (the RFC 142 migration
corpus) must stay green. Say so by name.

## D — the operator warning

At startup, **one warning per provider** whose `validation_status` is
`requires_review`, logged once, in operator words. It names the provider by
slug, says **federated sign-in through it is off until an administrator
re-checks and re-enables it**, and says where. *Where* does not exist yet: S5 builds
it. So for now, name the operator guide's section S7 will write, and **mark the
sentence with a `TODO(096-B2 S5)` comment in the code**, so the reviewer of S5
updates it. **A test asserts the line's text and level**, the way RFC 143's
CA-bundle warning test does (`crates/sui-id/tests/e2e/r143_no_ca_certificates.rs`).
**If that test starts a server, give it its own OS-assigned port**, as that
file now does. A fixed shared port is a race.

## E — records

- `CHANGELOG.md` `[Unreleased]`, **`### Changed`**, in operator words, leading
  with the consequence: on upgrade, every federation provider is disabled until
  re-enabled, and why. Mark it clearly that re-enabling arrives later in the
  same release, since S1 alone must never ship.
- **No `rfcs/` edits.** The contract rows for C17/C18/C20/C23 are untouched
  this stage.

## F — constraints and what to return

- **No tag, no dispatch.** `bash scripts/local-matrix.sh --changed` (the complete
  matrix: a migration is Rust scope), quoting the scope line and **the names of
  any failing checks**. G22 by name. **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/096-b2-stage1-schema-and-the-upgrade-2026-11-11.md`,
with:
- the baseline and the migration number;
- the migration;
- **what "invalidate" means per table, and the source line it rests on**;
- the fixture list, each test with what it seeds (not only what it asserts);
- the mutation;
- the warning's exact text.
