# RFC 096-B2 stage 4 — C17, enable and disable

> ## Landed 2026-11-12, with reviewer fold-ins. Next:
> [`096-b2-stage5-the-admin-surface-2026-11-12.md`](096-b2-stage5-the-admin-surface-2026-11-12.md).
>
> Cycle L, 096-B2 S4 —
> [`096-b2-stage-plan-2026-11-11.md`](096-b2-stage-plan-2026-11-11.md) §3.
> **This document is the dispatch.**

**Dispatched.** 2026-11-12, by the architect.
**Governing RFC.** [RFC 096](../../accepted/096-upstream-oidc-federation-validation.md),
**Accepted** (re-accepted 2026-11-11 with the C17-seam amendment, `:390` and
`:440`). RFC 094's C17 text: `rfcs/done/094-transactional-audit-registry.md:506-543`.
Building is permitted.
**Baseline.** The tip of `main` when you start (`git log -1`; record it). S3
must be in it, with its review's fold-ins.
**Read first:**
- the approved design: [`096-b2-c17-consumes-the-capability-decision-2026-11-11.md`](096-b2-c17-consumes-the-capability-decision-2026-11-11.md)
  §3. **It is the spec**: items 1–7, as approved;
- RFC 096 `:440-450` and RFC 094 `:515-543`, turned into the checklist in §A;
- the S3 review's §3, `.git-exclude/reviewed/096-b2-stage3-the-preflight-capability-2026-11-11.md`.

## 0 — what this stage is

**After this stage a disabled provider can be checked and enabled, and an
enabled one disabled**, through commands an administrator's action will call
in S5. No admin surface is built here; S5 builds it. Every operator-facing
refusal gets its message now, because S5 only renders them.

## A — C17 enable, RFC 096 `:440-446` and RFC 094 `:515-541`, as a checklist

| # | Clause | Where it lands |
|---|---|---|
| 1 | *"consumes that capability"*, via the C17-seam amendment | **one** `sui-id` function in `federation_preflight.rs`, `check_and_enable(app, admin, provider_id)`. It runs `preflight_provider` and moves the capability straight into the store command. Nothing else calls the store command (§C) |
| 2 | *"After `BEGIN IMMEDIATE`"* | **The store's Class-A runner opens every transaction `DEFERRED`** (`backend.rs:165`). C17 is the only command that names `IMMEDIATE`. Give the runner a way to open C17's transaction `IMMEDIATE`, and **change no other command's mode**. Show it with a test (§E) |
| 3 | *"captures `guard_at`"*, *"its trusted clock"* | read **once**, inside the transaction, from the store's own clock (`chrono::Utc::now()`, the source the preflight's `observed_at` uses). A seam under `cfg(any(test, feature = "test-support"))` only, so tests can place `guard_at`. **Production has no way to set it** |
| 4 | `0 <= guard_at - observed_at < 600s`; *"exactly 600 is stale"* | tests at 599 s (accepted), 600 s (stale), and a negative age, i.e. `observed_at` in the future (stale) |
| 5 | rechecks disabled, non-deleted, exact version, digest and generation | the digest is **recomputed** from the stored row (`from_stored` plus `compute_policy_digest` over the stored envelope) and compared with `ct_eq` to the capability's. Never trust the capability's digest alone |
| 6 | *"still configured"* | held by construction, as the amendment states (`:440`). Nothing to build; say so in a comment at the call site |
| 7 | checked-increments the generation | `checked_add`. At `i64::MAX` it refuses and rolls back; that needs a test |
| 8 | stores evidence bound to the new generation; enables | one guarded `UPDATE`: `enabled = 1`, `validation_status = 'validated'`, `activation_generation` = new, and every `evidence_*` column, with `evidence_activation_generation` = the **new** generation. 0051's CHECK must accept the row; a test shows that it does |
| 9 | `federation.provider.enabled`, with *"version, old/new activation generation, and bounded fingerprint identifiers"*; *"arbitrary caller time cannot enter the event"* | the fingerprints as hex. **No `observed_at`, no digest** (RFC 096 `:436-437`: the digest is never audited). Administrator actor, provider target, and the step-up evidence K01 records (`commands.rs:214`) |
| 10 | first of several same-generation capabilities wins | two preflights, then two enables: the second refuses as *changed since the check* |
| 11 | stale, replayed, wrong-policy, wrong-generation, deleted or already-enabled input rolls back without an event | one test each. "Replayed" is the same evidence submitted twice at store level, since a capability cannot be used twice by type |

## B — C17 disable, RFC 096 `:446-450` and RFC 094 `:520-524, :541-543`

- Requires a currently **enabled** row. It checked-increments the
  generation, clears every `evidence_*`, sets `enabled = 0`, and invalidates
  flows through S2b's `invalidate_provider_flows_within_tx`, with the event in
  the same transaction.
- `federation.provider.disabled` carries the old and new generation and the
  three counts, plus the administrator actor and step-up evidence.
- **Already disabled:** nothing is committed, no generation is burned (RFC
  096 `:449-450`), and the refusal says so.
- `validation_status` stays as it is. A disabled, `validated` provider still
  needs a fresh check to be enabled, since evidence is cleared. Say so in a
  comment.

## C — the seam (approved design, items 3 and 4)

- **The store command takes one struct**, `PreflightEvidence`, of distinct
  types:
  - `ConfigVersion` and `ActivationGeneration`, newtypes over `i64`;
  - `MetadataFingerprint` and `JwksFingerprint`, newtypes over `[u8; 32]`;
  - S3's `PolicyDigest`;
  - `observed_at`.

  Swapping two values must fail to compile. Name the command so its contract
  is plain, for example `enable_federation_provider_from_preflight`. Its doc
  says who may call it, and why.
- **The source-scope test**, in the shape of
  `crates/sui-id/tests/e2e/r096_b1_stage10b_f03_scope.rs`, but wider. Across
  **every** crate's production source (`crates/*/src`, excluding test files
  and `tests/` directories), the store command's name may appear in exactly
  two files: its definition in `sui-id-store`, and
  `crates/sui-id/src/http/federation_preflight.rs`. **Mutation:** name it in
  a third file, and quote the test going red. Then revert.

## D — refusals, each with its next step (approved design, item 6)

`check_and_enable` returns one closed enum: S3's `PreflightError` variants,
plus C17's own. Each message gives the reason **and** the next step, and a
test pins both halves, as S3's review fold-in did:
- **changed since the check**: run it again;
- **the check is more than 10 minutes old**: run it again;
- **already enabled**;
- **no longer exists**: reload the list;
- **storage**: try again, and see the server log.

Disable has its own: **already disabled**, **no longer exists**,
**storage**. **No message carries a fingerprint, digest, URL or upstream
body.**

## E — tests

- **The immediate transaction.** Hold a write lock from a second connection
  on a **file** database. C17 must wait for it, and its `guard_at` must be
  later than the moment the lock is released. A `DEFERRED` transaction would
  read first. Show the test red against `DEFERRED`.
- **End to end, the reason this exists.** `check_and_enable` against the mock
  upstream. Then **a real federated sign-in through that provider
  succeeds**, using the harness's own sign-in path. Then disable: an
  attempt started before the disable cannot complete, and a new sign-in is
  refused.
- **Rollback.** The failure injector for each branch of C17. G20 counts
  them (`check-m2a-rollback-coverage.sh`).
- **Every row in §A and §B** is named in the return with its test.

## F — records

- **Contract:** C17's row moves from `target-absent` to `implemented`, with
  its `files`, `mutation_surface`, `test_id`, events and descriptors. Update
  `write-authority.toml`, the registry and `audit-coverage-matrix.md`. G23
  stays green.
- **Docs:** add both events to `docs/src/reference/audit-events.md`.
- **No `CHANGELOG.md` entry yet.** No operator can reach these commands
  until S5. Say so in the return.
- **`create_enabled_for_test`'s doc** says no production code writes an
  enabled row. That is no longer true; correct it.

## G — constraints and what to return

- **No tag, no dispatch.**
- Run `bash scripts/local-matrix.sh --changed`, the complete matrix, and
  quote the scope line and any failing check names.
- **Don't edit files while the matrix reads them.**
- **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/096-b2-stage4-c17-enable-and-disable-2026-11-12.md`,
with:
- the baseline;
- **§A's and §B's tables filled in**: each clause, the `file:line` that builds it, and its test;
- how the runner opens C17 `IMMEDIATE`, and the test that shows it red against `DEFERRED`;
- the clock seam, and why production cannot reach it;
- the struct and newtypes;
- the scope test and its mutation;
- every refusal's exact text;
- the end-to-end sign-in test;
- the rollback tests, by name.
