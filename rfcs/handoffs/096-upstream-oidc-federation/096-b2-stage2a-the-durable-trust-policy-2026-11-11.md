# RFC 096-B2 stage 2a — the durable trust policy

> ## Whose turn it is: **the implementation role's.**
> Cycle L, 096-B2 S2a —
> [`096-b2-stage-plan-2026-11-11.md`](096-b2-stage-plan-2026-11-11.md) §3, and §6
> for why S2 split. **This document is the dispatch.**

**Dispatched.** 2026-11-11, by the architect.
**Governing RFC.** [RFC 096](../../accepted/096-upstream-oidc-federation-validation.md),
**Accepted**, §096-B2 with amendment A4 **as corrected 2026-11-11**. Building
is permitted.
**Baseline.** The tip of `main` when you start (`git log -1`; record it).
**Read first:**
- the stage plan's **§6**, which measures what is and is not stored today;
- RFC 096 `:355-372` (the trust-configuration table and the canonical-URL rule), `:424-435` (the digest's nine fields), and `:1016-1036` (the migration rule);
- `architecture.md` `:239` and `:306-307`;
- `verification.md` §7.

## 0 — what this stage is, and is not

**This stage makes the provider's trust policy durable, typed, and the only
thing the login path trusts.** After it, every field of the digest is stored
or derived from stored values. The policy is built by one validator. The
database itself refuses an enabled provider without current, validated
evidence.

It adds **no command id**. C23, C24's retirement, the display-label update and
A2's removed-block disable are all S2b. The policy digest itself is S3.

**Why it comes before C23:** C23 compares the stored policy with the
configured one and replaces it. Today `id_token_algs` is not stored at all:
the callback reads it from the config file by slug (`federation.rs:1084-1108`).
So a config edit changes what a provider accepts with no review, and there is
nothing for C23 to compare against. This stage closes both.

## A — migration 0051: store the algorithms, and make "enabled" mean "validated"

**Measure the next free number first** (`0051` at the plan's baseline), and
record it. **Never edit 0050.** `migrations.rs` records the applied *version
number* only (`:4`), so an edited 0050 would never run on a database already
at 50. The owner's local one is.

**It is a table rebuild**, because SQLite cannot add a CHECK to an existing
table. Use the runner's existing mechanism: the `-- MIGRATION:FK_DISABLE_REQUIRED`
first line (`migrations.rs:611`). `0022_boolean_checks.sql` is the precedent.
The runner turns foreign keys off *before* the transaction, which is the
only place SQLite honours it. **Without that marker, the rebuild's DROP
cascades through `federation_link` and deletes every link**
(`0038_federation_link.sql:22`).

**1. The new column.** `id_token_algs TEXT`, **nullable**. NULL means
*not recorded*. RFC 096 `:1027` says the migration *"writes no guessed
… algorithm"*, so every migrated row gets NULL. A non-NULL value is the
canonical encoding from §B: tokens from the closed set, in the set's
fixed order, single-space separated. The SQL CHECK only needs to refuse an
empty string. **The typed decoder is the authority**, and a stored value it
refuses makes the row runtime-ineligible (§D), never a crash.

**2. The invariant, as a table CHECK.** Use this expression, not a prose
restatement of it:

```sql
CHECK (
  enabled = 0 OR (
    validation_status = 'validated'
    AND id_token_algs IS NOT NULL
    AND evidence_config_version = config_version
    AND evidence_activation_generation = activation_generation
    AND evidence_policy_digest IS NOT NULL
    AND evidence_observed_at IS NOT NULL
    AND evidence_metadata_fingerprint IS NOT NULL
    AND evidence_jwks_fingerprint IS NOT NULL
  )
)
```

This is RFC 096 `:1034-1035` (*"an enabled legacy-shaped or unvalidated row
is runtime-ineligible"*), held by the database rather than by every writer
remembering it. C17 enable (S4) stores evidence *"bound to the new
generation"* (RFC 094 `:518-520`), so the equalities hold for every row C17
will produce.

**3. Every row is copied with `enabled = 0`.** No row can meet the invariant
before S4 exists, because nothing writes `validated`. A row that is enabled
today can only be one C16 created after 0050 (§C), on an unreleased build.
Copying it as enabled would fail the CHECK and abort the migration. Say so
in the migration comment.

**4. Keep everything else exactly.** That means every column, CHECK, DEFAULT
and index from 0037, 0045 and 0050, and every row's id, slug, sealed secret,
counters, status and evidence. The runner's `foreign_key_check` after commit
must find nothing.

**Tests, each in a sibling test file:**
- **The rebuild keeps the children.** Seed a pre-0051 database with a
  provider that has links, an attempt, an MFA continuation and a ceremony.
  After 0051, assert that every child row survives, that ids are unchanged,
  and that the sealed secret still decrypts to the same value. Count before
  and after.
- **The CHECK, both sides.** An enabled row with everything present is
  accepted. Then one test per conjunct: each field missing, or
  each mismatched counter, is refused. **Mutation:** delete one conjunct and
  its test goes red. Quote which.
- **The C16 shape migrates.** Seed an enabled, `requires_review` row with no
  evidence before 0051, the row C16 creates today. Assert that 0051 succeeds
  and the row is disabled after it.
- **G22** (`cargo +stable test -p sui-id-store --lib tests_rfc142 --locked`)
  stays green, with the new migration in its corpus. Say so by name.

## B — one typed trust policy, built by one validator

A type that holds the digest's nine fields (RFC 096 `:424-428`), with
**private fields and one validating constructor**. Put it in `sui-id-store`,
because the store persists it and S3's digest is computed from it. A
`federation_policy` module is fine; tests go in a sibling file.

| Field | Stored as | Rule (RFC 096 table row) |
|---|---|---|
| issuer | `issuer` | `:355`, but see **the issuer rule** below |
| client ID | `client_id` | `:356`: 1–512 UTF-8 bytes, no control characters |
| client secret | `client_secret_enc` | `:357`. **Presence only in this stage.** S2b compares values |
| token-auth method | **derived** | `:358`: `client_secret_basic` when a secret is present, else `none`. The same rule the callback applies today (`federation.rs:949-954`) |
| scopes | `scopes`, canonical | `:359`: a unique subset of `openid email profile`, `openid` mandatory, at most three |
| ID-token algorithms | **`id_token_algs`, new** | `:360`. Move `validate_id_token_algs`'s rules (`config.rs:462`) into this constructor. Do not keep two copies |
| endpoint origins | `allowed_origins` | `:361`: 1–8 canonical HTTPS origins, **the issuer's origin included**. An empty config list means *the issuer's origin alone*, the meaning it has today (`discovery.rs:354-355`) |
| HTTPS ports | **derived** | `:362`: the set of the origins' effective ports. The stage plan §6 says why it is derived and not a setting |
| provision mode | `provision_mode` | `:363`: exactly `link_only` or `provision_on_first_login` |

**Three things to get exactly right:**

1. **The provision mode stops guessing.** `ProvisionMode::parse` maps every
   unrecognised value to `link_only` (`models.rs:162-166`). Make it fallible.
   Measure **every** caller, including the database row decoder: a stored
   value it cannot read is a malformed row, not `link_only`. List the callers
   in the return.

2. **The canonical-URL rule, for origins.** An origin input must equal
   `url::Url::parse(s)?.origin().ascii_serialization()` byte for byte. That
   refuses `https://Example.com`, `https://example.com:443` and
   `https://example.com/`. Each of the three is a test.

3. **The canonical-URL rule, for the issuer.** RFC 096 `:355` says the
   issuer's *"exact serialized value is authoritative"*, and `:600` compares it
   byte-for-byte with discovery's `issuer`. **But `url` serializes an empty
   path as `/`**, and real issuers have none: Google's is
   `https://accounts.google.com`. A plain *"must equal the serialization"*
   rule would refuse it. The rule, as an expression:

   ```text
   let u = Url::parse(s)?;
   u.scheme() == "https"
     && u.username().is_empty() && u.password().is_none()
     && u.query().is_none() && u.fragment().is_none()
     && (u.as_str() == s || (u.path() == "/" && u.as_str().strip_suffix('/') == Some(s)))
   ```

   **Check the last line against the parser before relying on it**: what
   `as_str()` returns for `https://accounts.google.com`, for
   `https://accounts.google.com/`, and for `https://accounts.google.com/a/`.
   The trailing-slash exception must apply only to an empty path. Tests:
   - `https://accounts.google.com` and `https://login.example.com/tenant/v2.0` are **accepted**;
   - `https://Accounts.google.com`, `https://accounts.google.com:443`, `https://accounts.google.com/a/../b`, `http://…`, and one with a query, one with a fragment and one with userinfo are **refused**.

   This is the architect's reading of `:355` with `:367-372`, not a new rule.
   If the parser behaves otherwise than this assumes, **stop and report**
   rather than adapt the expression.

**The issuer's host rule is not in this stage.** That rule is "no IP literal,
no `localhost`, `.localhost`, `.local`, `.internal` or `.home.arpa`".
Enforcing it would break every e2e test that mocks its upstream at
`https://127.0.0.1:PORT`. The discovery module recorded the same cost once
already (`discovery.rs:81-115`). **Measure it and report it; don't build it:**
- which test files configure an IP-literal or reserved-name issuer, and how many tests each holds;
- whether `serve_raw_https`'s named-host-plus-resolver approach could replace them.

It gets its own stage, planned from your numbers. Leave a
`TODO(096-B2 issuer host rule)` at the one place the check will go.

**Where it runs.** `Config::validate` builds the policy for every
`[[federation_providers]]` block. **Malformed configuration refuses startup**
(RFC 096 `:1032`), as `validate_id_token_algs` already does. **Every refusal
names the provider's slug, the field, and the rule it broke**, in the style of
`config.rs:473-516`'s messages. Replace the origins checks at `config.rs:278-302`
with the constructor too. **One validator, not two.**

**This is a breaking change for some existing configuration files.** List
in the return every rule this stage newly enforces, with its exact refusal
message. Each one gets a line in the CHANGELOG.

**Reading a stored row.** Reading a row yields the typed policy, or "no usable
policy" when `id_token_algs` is NULL or any stored value fails the
constructor. Never a default.

## C — C16 creates a disabled provider, and only tests can make one enabled

- **`create_within_tx` loses its `enabled` input.** The production create
  path then has no way to insert an enabled row: a type fact, not a
  convention. Today C16 copies it from config (`startup.rs:421`).
- C16 stores the **validated** policy, including `id_token_algs` and the
  canonical scopes. C16's event is unchanged.
- **Test fixtures.** About ten build an enabled provider directly. That
  includes `create_within_tx` with `enabled: true` in the store's repo and
  runner tests, `federated_mfa/tests.rs`, `federation_attempt_terminalize/tests.rs`
  and `tests/e2e/federation_fail_closed.rs`. Plus `set_enabled`
  (`repos/federation_provider.rs:253`, gated `#[cfg(any(test, feature =
  "test-support"))]`). The CHECK refuses all of them now. Replace them with
  **one** test-support builder, under that same gate, which writes an
  enabled, `validated` row with evidence bound to the row's current version
  and generation. **It must satisfy the CHECK, not bypass it.** List every
  fixture you moved.
- Measure whether `set_enabled`'s seam appears in `contracts/write-authority.toml`,
  and keep that file true.

## D — the callback trusts the stored policy

- `federation.rs:1108` reads `provider_config.id_token_algs` from the config
  file. **Read the stored, typed policy's algorithms instead.** A provider
  with no usable stored policy is refused, fail-closed, with the existing
  denial shape. Name the reason you used in the return.
- **Leave the config-presence lookup at `:1084` as it is.** It still refuses
  a provider whose block is gone. S2b decides its future together with A2.
- **The test, and its mutation:** configure a provider whose config-file
  algorithms differ from its stored ones. Assert that the stored ones decide.
  Then point the read back at the config file and quote the test going red.

## E — the `enabled` key in the config file

It no longer enables anything: enabling is C17's job, after a preflight
(S4). **The key stays accepted for now.** When a block sets `enabled = true`,
startup logs **one warning per provider**, in operator words. The warning
names the provider and says the key no longer enables it, and how a provider
is enabled. *How* is S5's, so mark the sentence `TODO(096-B2 S5)`, as S1 did.
A test asserts the warning's text and level. Refusing the key outright is a
later decision, not this stage's.

**Correct the prose that tells operators to set it.** That is
`docs/src/reference/configuration.md` `:257-279`, and
`docs/src/guides/operators.md` `:1715-1716`. Make the smallest change that
is true; S7 writes the full guide. `grep -rn "enabled" docs/src` for any
other federation sentence that the change falsifies.

## F — the upgrade report says what `architecture.md` asks for

S1's warning (`startup.rs`, `warn_providers_requiring_review`) names only the
slug. `architecture.md` `:306-307` asks for *"internal ID, slug, whether a
link exists, and missing policy fields but no secret, issuer query, email, or
subject."* Add the internal ID, a yes/no for links, and the missing fields:
after this stage, `id_token_algs` when it is NULL. Keep it **one warning per
provider**, and keep the `TODO(096-B2 S5)`. Update S1's test to assert the new
text, and add one case with a link and one without.

## G — records

- `CHANGELOG.md` `[Unreleased]`, under **`### Changed`**, in operator words:
  - the newly enforced configuration rules, listed;
  - `enabled` in the config file no longer enables a provider;
  - the algorithms a provider accepts are now the stored ones, so a config edit alone no longer changes them.
- **Contracts:** C16's row (`mutation_surface`, `test_id`) if they change, and
  `write-authority.toml` if the seam moves. G23 (`cargo xtask
  audit-structure`) stays green.
- **No `rfcs/` edits.**

## H — constraints and what to return

- **No tag, no dispatch.**
- Run `bash scripts/local-matrix.sh --changed`. A migration is Rust scope, so
  this runs the complete matrix. Quote the scope line and **the names of any
  failing checks**.
- G22 and G23 by name.
- **Hunk hashes beside the package.**

**Return:** `.git-exclude/review-requests/096-b2-stage2a-the-durable-trust-policy-2026-11-11.md`,
with:
- the baseline, and the migration number;
- the migration, and the CHECK as written;
- the rebuild test: **what it seeds**, with counts before and after;
- the per-conjunct CHECK tests, and the mutation you ran;
- for the issuer rule: what `as_str()` returned for the three probe inputs, and the accepted and refused cases;
- every newly enforced configuration rule, with its exact message;
- every `ProvisionMode::parse` caller, and what each now does with an unknown value;
- the fixtures you moved to the builder, each with what it seeds;
- the callback test and its mutation;
- the two warnings' exact texts;
- **the issuer-host-rule measurement** (files, test counts, and whether `serve_raw_https` fits).
