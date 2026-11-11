# RFC 096-B2 stage 2b — C23 at startup

> ## Whose turn it is: **the implementation role's.**
> Cycle L, 096-B2 S2b —
> [`096-b2-stage-plan-2026-11-11.md`](096-b2-stage-plan-2026-11-11.md) §3.
> **This document is the dispatch.**

**Dispatched.** 2026-11-11, by the architect.
**Governing RFC.** [RFC 096](../../accepted/096-upstream-oidc-federation-validation.md),
**Accepted**, §096-B2 with amendments A2 and A4 (A4 **as corrected
2026-11-11**). RFC 094's C23 design: `rfcs/done/094-transactional-audit-registry.md:547-575`.
Building is permitted.
**Baseline.** The tip of `main` when you start (`git log -1`; record it). S2a
must be in it.
**Read first:**
- RFC 094 `:547-575`, the governing text for C23. §A turns it into a checklist; build against both;
- RFC 096 `:164-174`, A2 and the corrected A4;
- `federation_policy.rs` from S2a, especially `from_stored` and `canonical_scopes`;
- the S2a review, `.git-exclude/reviewed/096-b2-stage2a-the-durable-trust-policy-2026-11-11.md` §2–§3.

## 0 — what this stage is

**After this stage, editing a provider's trust policy in the config file
takes effect on the next boot.** It takes effect through C23, which leaves the
provider disabled until it is re-checked and re-enabled. Startup no longer
logs "differs" warnings for those fields, because nothing is left unapplied.
The stage also:
- **retires C24** into C23;
- adds **C26**, the display-label update, which does not disable;
- adds **C25**, A2's disable of a provider whose block was removed.

**One rule throughout:** compare **decoded** values, never column strings.
S2a's review found three places where spelling differences caused writes or
warnings for changes nobody made (§3 of that review).

## A — C23, RFC 094's text as a checklist

Every line below is a clause of RFC 094 `:547-575`. **Each one is built here.**
If one cannot be built, stop and report it; do not reword it.

| # | Clause | Where it lands |
|---|---|---|
| 1 | *"validates the expected current version and generation"* | the guarded `UPDATE`'s `WHERE` (row 9) |
| 2 | *"checked-increments both `config_version` and `activation_generation`"* | `checked_add`. Overflow rolls back (row 8) |
| 3 | *"replaces the complete typed trust policy"* | every policy column in one `UPDATE`, from a `FederationTrustPolicy` built from the config |
| 4 | *"sets the provider disabled/review-required"* | `enabled = 0`, `validation_status = 'requires_review'`, every `evidence_*` set to NULL |
| 5 | *"invalidates every pending/exchanging login attempt, federated-MFA continuation, and bound ceremony"* | §B's function, with its counts |
| 6 | *"appends `federation.provider.policy_updated`"* | same transaction |
| 7 | Event attributes: *"old/new version and activation generation, sorted changed-field enum names, bounded invalidated login-attempt/MFA/ceremony counts, and resulting `disabled/requires_review` state"*. They *"exclude issuer/origin/client identifiers, secret material, and configuration values"* | the descriptor and a note-field test |
| 8 | *"Missing/malformed/overflowed counter, predicate loss, attempt invalidation failure, event failure, or audit failure rolls back all database effects"* | one rollback test per cause you can inject. The failure injector is RFC 096 `:45`'s closure evidence |
| 9 | *"exact provider ID, exact old version and activation generation, non-deleted row … exactly one provider row must change"* | `WHERE id = ? AND config_version = ? AND activation_generation = ?`. Anything but one changed row rolls back |
| 10 | *"Cache eviction occurs only after commit"* | at startup the cache is empty. Say so in a comment at the call site. **Don't add an eviction call that does nothing** |
| 11 | *"runs after migrations and audit-chain readiness but before the affected provider can be routed or preflighted"* | where reconcile already runs in `prepare`. Confirm in the return that nothing serves before it |
| 12 | *"sealed system actor is `StartupConfiguration`; the target is internal provider ID"* | the same system-actor shape C16 and C24 use (`commands.rs`, `for_system_actor`). If a named `StartupConfiguration` actor type is expected and missing, report it |

**The changed-field set.** It uses the nine digest fields' names, sorted:
`client_id`, `client_secret`, `endpoint_origins`, `https_ports`,
`id_token_algs`, `issuer`, `provision_mode`, `scopes`, `token_auth_method`.
Compare field by field, against the stored value **decoded** by S2a's rules.
**A stored field that is NULL or fails to decode counts as changed.** So
every row 0051 copied runs C23 once, on its first boot after this stage. That
is correct: it records `id_token_algs` for the first time, and the provider is
already disabled. **C23 runs exactly when the set is non-empty.**

**The client secret.** The config holds an environment variable's name, and
`cfg.resolve_secret()` returns the value. The database holds a sealed
envelope. **Compare plaintexts in constant time** (`subtle` is already a
dependency of `sui-id-store`), after `crypto::open` with
`federation_provider::AAD`. The cases:
- **Stored absent, configured present (or the reverse):** changed. So is
  `token_auth_method`, which follows from presence.
- **Stored envelope fails to open:** changed, and resealed.
- **Plaintexts differ:** changed, and resealed.
- **Plaintexts equal:** unchanged, and **not** resealed. A reseal changes the
  envelope, and RFC 096 `:426` puts the envelope's hash in the digest. So a
  needless reseal would invalidate evidence for nothing.

**`https_ports` and `token_auth_method` are derived.** Each changes exactly
when the field it is derived from changes in a way that moves it. Test one
case of each: an origin moving from `:443` to `:8443` changes
`endpoint_origins` and `https_ports`. A secret removed changes `client_secret`
and `token_auth_method`.

**Origins storage.** Keep the 0045 encoding, as S2a's fold-in 1 does: the
operator's list, or `""` for the issuer's origin alone. The comparison is on
decoded effective sets, so `""` and `"https://issuer.example"` on an issuer of
`https://issuer.example` are **unchanged**. That is a test.

**The log line C23 writes.** In operator words, once per provider, at `warn`.
It names the provider and the changed fields, and says the provider is now
disabled until it is re-checked and re-enabled. Mark the "how" with
`TODO(096-B2 S5)`, as S1 and S2a did. **No values in it**, for the same reason
as the event (row 7).

## B — one function that invalidates a provider's flows

C23, C25, and later C17's disable (S4) and C18 (S6) all need it. **Build it
once, here.** Put it in `sui-id-store`, taking the transaction and the provider
id, and returning the three counts. It is migration 0050's upgrade SQL
(`0050_federation_provider_validation_schema.sql:184-224`), scoped to one
provider:
- attempts in `pending` or `exchanging` become `failed`, with `claimed_at`
  coalesced, exactly as at `:184-187`;
- ceremonies for the provider are deleted (`:207`);
- MFA continuations in `pending` become `invalidated` with the continuation
  cleared (`:221-224`).

**Where a table already has a one-row transition** (`federation_mfa_pending::
invalidate_within_tx`, `:434`), match its semantics exactly, and say in the
return how you made sure. Read 0050's comments on the ordering. **A test
seeds two providers' flows and shows that only one provider's are touched**,
with exact counts.

## C — C24 retires into C23

- **The command.** `update_federation_provider_allowed_origins`, its
  descriptor, and `update_allowed_origins`/`update_allowed_origins_within_tx`
  stop being the way origins change. C23's `UPDATE` writes them. Delete what
  then has no caller. Each test of C24 either moves to C23 or is deleted
  because C23's tests cover it; **list each one, and say which**.
- **The contract.** Remove C24's row from `contracts/write-commands.toml`,
  and add a header comment as F01's and C12/C13's have (`:32-45`). Add a
  **"Retired: `C24`"** paragraph to
  `rfcs/handoffs/094-transactional-audit/command-inventory.md`, in the style
  of `:23-66`. Keep `write-authority.toml` true.
- **The event.** `federation.provider.allowed_origins_changed` is no longer
  written. Add it to `docs/src/reference/audit-events.md`'s **Retired events**
  (`:297`), saying it was replaced by `federation.provider.policy_updated`, and
  update `contracts/audit-coverage-matrix.md`.
- **The registry variant.** Rows with this action already exist in real
  audit logs, and the hash chain covers them. **Measure, before removing
  `FederationProviderOriginsChanged`,** whether anything that reads or
  verifies a stored row needs the variant: chain verification, the audit
  viewer, or the label lookup. Report what you found, and keep the variant
  if anything needs it.

## D — C26, the display label

`display_name` is not trust policy (RFC 096 `:434`). A change updates it
without disabling, through **C26**, a new Class-A command with the
system-actor shape. Its event is `federation.provider.display_name_changed`,
with **no values in it**, the same rule as row 7. Contract row, registry
entry, a test of the write and its event, and a rollback test. When a boot
changes both trust policy and label, run C23 first, then C26, and say so in
a comment.

## E — C25, a provider whose block is gone (amendment A2)

After the loop over configured blocks, take every stored provider whose slug
no configured block names:
- **If it is enabled:** C25 disables it, with the same effect as C17's
  disable (RFC 094 `:520-524`): generation checked-incremented, evidence
  cleared, flows invalidated through §B, `enabled = 0`. **Links are kept.**
  The event is `federation.provider.removed_from_configuration`, with the
  old and new generation and the three counts. Contract row, registry entry,
  and tests: the write, the event, rollback, and **links unchanged**.
- **If it is already disabled:** C25 commits nothing and writes no event.
  A generation must never be burned for nothing (RFC 096 `:449-450`).
- **Either way**, one `warn` line per provider, every boot. It says the block
  is gone, the provider is disabled and keeps its links, and to restore the
  block, or delete the provider (`TODO(096-B2 S6)`).

**The callback's config-presence check (`federation.rs`, `ConfigDrift`)
stays.** Config is fixed for the life of the process, so after C25 an enabled
provider without a block cannot reach it. It costs one lookup and fails
closed, so keep it, and update its comment to say why.

## F — what goes away, and the records

- **`warn_if_federation_field_differs`** has no remaining use once every
  field is applied. Remove it with its tests, and say which tests went.
- **Docs.** `docs/src/reference/configuration.md`'s *"Which fields take
  effect"* note (S2a left it accurate for S2a): rewrite it to say what is now
  true. That is:
  - every trust field applies on the next boot and disables the provider until it is re-checked and re-enabled;
  - `display_name` applies without disabling;
  - removing a block disables the provider and keeps its links.

  `grep -rn "allowed_origins\|reconcil" docs/src` for sentences this
  falsifies, including RFC 134's.
- **`CHANGELOG.md`** `[Unreleased]`, under `### Changed`, in operator words.
- **G23** green. **No `rfcs/` edits** except `command-inventory.md`.

## G — constraints and what to return

- **No tag, no dispatch.**
- Run `bash scripts/local-matrix.sh --changed`, the complete matrix. Quote
  the scope line, and the names of any failing checks.
- G22 and G23 by name.
- **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/096-b2-stage2b-c23-at-startup-2026-11-11.md`,
with:
- the baseline;
- **§A's table filled in**: each clause, the `file:line` that builds it, and the test that proves it;
- the changed-field computation, and its tests per field. That includes the two derived ones, the secret's four cases, and the `""` origin case;
- §B's function, the two-provider test, and how you matched `invalidate_within_tx`;
- every C24 test, moved or deleted, and why;
- the registry-variant measurement;
- C25's and C26's rows, events and tests;
- every log line's exact text, captured from a run;
- **a mutation for each of**:
  - dropping the generation guard from C23's `WHERE`;
  - resealing an unchanged secret;
  - letting C25 commit for an already-disabled provider.

  Quote each test going red.
