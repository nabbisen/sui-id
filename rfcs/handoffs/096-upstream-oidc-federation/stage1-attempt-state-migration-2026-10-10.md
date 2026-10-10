# RFC 096-B1 stage 1 — migration `0046`, the `federation_login_attempt` table

**Dispatched.** 2026-10-10 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Plan.** [`096-b1-stage-plan-2026-10-09.md`](096-b1-stage-plan-2026-10-09.md),
authorized 2026-10-09. This is its stage 1.
**Prior.** Stage 0 complete — `ReadConn` exists, with the eight-member
side-effect class, `EXPLAIN`, and a comment-hardened `PRAGMA` refusal.
**Baseline.** Read the tip with `git log -1`, hash against it, name the full SHA.

## Scope: the schema, and nothing that uses it

RFC 096 `:577-589`, verbatim:

```text
id, provider_id, provider_config_version, provider_activation_generation,
state_sha256 UNIQUE, nonce_sha256, browser_binding_sha256,
pkce_verifier_sealed, exact_redirect_uri, next_path,
created_at, expires_at, status(pending|exchanging|completed|failed), claimed_at
```

**`:23` names migration evidence as a 096-B1 closure item**, which is why the
schema is its own stage and produced with its evidence rather than reconstructed
later.

**Stage 1 writes no repo functions and no command handlers.** Creating an
attempt is stage 2; claiming it is stage 3. If a column's purpose only becomes
clear when something writes it, say so in the package rather than adding the
writer here.

## Measured for you

- **The migration is `0046`.** `0045_federation_provider_allowed_origins.sql` is
  the current top.
- **`MAX_SCHEMA_VERSION` needs no manual bump** — `migrations.rs:205-215`
  computes it at compile time from the `MIGRATIONS` slice. Adding the entry is
  enough. Say if that is not what you find.
- **Registration is a `Migration { version: 46, sql: include_str!(...) }` entry**
  appended to that slice, matching `0045`'s shape.
- `MAX_SCHEMA_VERSION` is referenced by `errors.rs`, `tests_rfc112.rs`,
  `tests_rfc115.rs`, `backup/ops.rs` and `handlers/settings.rs`. **Check each
  before assuming the compile-time computation is the whole story** — a test
  that pins the current number would need updating, and I did not find one, but
  I grepped rather than read all five.

## Decisions to make and state, not to take from me

**Column types and constraints.** The RFC gives names and one constraint
(`state_sha256 UNIQUE`). Everything else — types, nullability, the `status`
representation, foreign keys to `federation_provider`, indices — is yours to
design. For each, say why. In particular:

- **`status(pending|exchanging|completed|failed)`.** A `CHECK` constraint, a
  lookup table, or a bare `TEXT`? Whatever you choose, an invalid status must be
  unstorable, not merely unwritten.
- **Hashes are "raw fixed-size values"** (`:587-588`). So `BLOB` of a fixed
  length rather than hex `TEXT`, unless you have a reason — and if `BLOB`, say
  how the length is enforced.
- **`pkce_verifier_sealed`** holds sealed bytes with AAD binding the attempt ID,
  provider ID, config version and activation generation (`:586-587`). Stage 2
  does the sealing; stage 1 decides where it lives.
- **`claimed_at` versus `status`.** Both encode progress. Say whether one is
  derivable from the other and whether the schema should prevent them
  disagreeing.
- **Does anything cascade when a provider is deleted or its version changes?**
  The attempt carries `provider_config_version` and
  `provider_activation_generation` precisely so a superseded attempt is
  identifiable; make sure the schema does not instead make it *vanish*.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The migration**, with every column's type and constraint justified in one
   line each.
2. **Migration evidence**, which `:23` requires for closure: applied forward
   against a database at `0045`, the resulting schema dumped, and the five
   `MAX_SCHEMA_VERSION` consumers checked.
3. **A test that an invalid `status` cannot be stored**, whichever mechanism you
   chose.
4. **Whether `0046` is reversible**, and if not, why that is acceptable here.
   I have not checked whether this project does down-migrations; find out rather
   than assume either way.
5. **Mutation evidence** for each constraint you add — drop it, show what fails.
   If a constraint has no test that notices its absence, it is decoration.
6. **Per-hunk SHA-256** against the tip you named.
7. **Gate evidence** from a throwaway clone with its own `target/`, plus
   `cargo test --workspace`.
8. **Anything you think is wrong with this dispatch.** You have corrected
   something of mine in each of the last three packages — the site count, the
   membership list's eighth member, and a hole in code I had told you not to
   touch.

**Not 096-B1's closure.** Stages 2–8 carry the rest of `:23`'s items.
