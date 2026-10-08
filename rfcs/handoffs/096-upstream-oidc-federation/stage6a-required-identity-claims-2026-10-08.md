# RFC 096-A stage 6a — the required identity claims: `iss`, `sub`, `aud`, `azp`

**Dispatched.** 2026-10-08, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Baseline.** `2a6c35d` (stage 5, Level B, 24/24 gates green).
**Prior stages.** 1, 2, 3a, 3b, 4a, 4b, 4c, 4d, 5 — all landed.

## Why there is a stage 6 at all

Stage 5's audit showed the corpus was already built, and in closing it I
checked 096-A against its closure prerequisite rather than against my own stage
plan. They disagree. RFC 096 `:21` requires nine attack categories *"each
handled as designed"*; four are not implemented — token substitution, nonce,
issuer and audience — because **the nine stages I planned for 096-A had no
claims stage**, although 096-A's scope sentence (`:59-61`) names *"required
ID-token claim validation; the mandatory one-time nonce"*.

That omission is mine, not a defect in any returned stage. Your stage 5 package
placed those rows in 096-B1, which was a reasonable reading of the plan you
were given; the RFC puts them here, and splits the nonce precisely — `:65-66`
gives 096-A *"the nonce validation rule"* and 096-B1 only *"the durable attempt
state that makes a nonce genuinely one-time … because it is a mutation."*

So 096-A has five stages left: **6a** (this one), 6b (time claims), 6c (bounded
optional claims and the construction capability), 7 (the nonce rule), 8 (the
corpus rows stage 5 could not cover). **Do not attempt more than 6a.**

## Scope — four claims, no more

RFC 096 `:656-659`, verbatim:

| Claim | Rule |
|---|---|
| `iss` | Required string, byte-exact configured issuer |
| `sub` | Required string, 1–255 UTF-8 bytes, no control; preserved exactly |
| `aud` | Required string or 1–8 unique strings; contains exact client ID |
| `azp` | Required and exact client ID when `aud` has multiple values; when present for one audience, also exact |

Nothing else. `exp`/`iat`/`nbf` are 6b even though `exp` is already checked by
the library default — leave that default exactly as it is and do not touch
`validation_for`'s `exp` behaviour. The optional claims (`email`,
`preferred_username`, `name`, `amr`, `acr`, `auth_time`, `at_hash`) are 6c.

**`aud` is the one existing line you must change.** `id_token.rs:399` sets
`validation.validate_aud = false`, and its comment says audience matching is
*"out of this stage's scope"* — true when stage 4a wrote it, and this is the
stage that takes it. Decide deliberately whether to bind `aud` through
`jsonwebtoken`'s own `set_audience` or in our own validator, and **say which
and why in the package.** Read `validation.rs`'s match on
`(Audience::Parsed(_), None)` before you choose — stage 4a's comment records
that the library rejects *every* token carrying `aud` when `validate_aud` is
left true with no expected audience set, so the two options are not
interchangeable. If you keep our own validator, rewrite that comment: it must
not still say audience matching is out of scope once this stage lands.

## Design constraints

**No durable mutation.** 096-A `:63-66`. The expected issuer and client ID are
**parameters**, exactly as `verify_id_token_against_jwks` takes the JWKS as a
parameter. No database read, no provider lookup, no session. This is what makes
the stage testable without any of 096-B1's machinery.

**Every rejection names itself.** Follow the established pattern — a distinct
error variant per rule, not a boolean. Tests assert the specific variant, never
`is_err()`. Stage 5's table is the standard: a reader must be able to point at
the test for each row.

**`sub` is preserved exactly.** Validate its bounds; do not normalise, trim or
case-fold it. RFC 096 `:693` makes `(provider_id, sub)` the sole lookup key, so
any transformation here silently changes identity.

**Duplicate and type discipline.** A claim present with the wrong type, a
duplicate member, an invalid character or an exceeded bound is a rejection, not
a partial ignore. `aud` as a JSON number, `aud` as an empty array, `aud` as 9
strings, a repeated `iss` member — each refused by its own name.

Note the citation carefully: RFC 096 states this rule explicitly at `:679-681`
for the **optional** claims, and at `:673-676` for NumericDate values. Neither
sentence is about the four required claims in this stage. I am asking you to
apply the same discipline here on consistency grounds — stages 2 and 3a already
refuse a repeated member by its own name
(`a_repeated_declared_member_is_refused` in both the header and JWKS modules),
and a required claim cannot sensibly be *less* strict than an optional one. If
you think that inference is wrong, say so rather than implementing it; it is
mine, not the RFC's.

**Byte-exact means byte-exact.** No URL normalisation on `iss`: no trailing-slash
tolerance, no case folding on the host, no percent-decoding. If the configured
issuer and the claim differ by one byte, refuse.

## Where it goes

Tests live **beside, not inside** (RFC 137): `crates/sui-id/src/http/<module>/tests.rs`,
declared with the `#[path]` pattern the existing modules use. Decide whether
these claims validate inside `id_token.rs` or in a new sibling module and
justify it in one line — `id_token.rs` is already large, and 6b/6c/7 will add
to whatever you choose.

If you add any root-level config file, register it in the Rust lanes' shared
`paths` in `contracts/gate-inputs.toml` or **A3.4 condition 9 fails**. I do not
expect this stage to need one.

## What to return

A working tree (not a commit), plus a package under
`.git-exclude/review-requests/` containing:

1. **The row-to-test table**, in stage 5's format — each of the four claims'
   rules mapped to the test that refuses each violation, with the module each
   test lives in.
2. **Mutation evidence.** For each of the four claims, break the rule in the
   source and show which test fails and with what message. Stage 5 proved the
   value of the sharp mutation over the convenient one: pick the mutation that
   isolates the rule under test. Copy the file to scratch first and restore
   from the copy — never `git checkout --`, which would discard your own
   uncommitted work.
3. **The `aud` decision**, as described above, with the reasoning and the
   reading of `validation.rs` that supports it.
4. **Per-hunk SHA-256**, from `python3.14 scripts/hunk-hashes.py --baseline 2a6c35d`,
   pasted verbatim.
5. **Gate evidence.** The gates refuse a dirty tree, so run them in a throwaway
   clone under `.git-exclude/tmp/clones/` with its own `target/`, as you did
   for stage 5. Keep `CARGO_TARGET_DIR` inside that clone — `ci-gate.sh`
   refuses one outside `--root`, and an absolute `/tmp` target has filled the
   disk before.
6. **Anything you think is wrong with this dispatch.** Stage 5's scope
   correction came from me reading the RFC against my own plan; stage 4d's fix
   came from you reading mine. Both are the process working.

**Do not claim 096-A's closure prerequisites.** Four of nine categories remain
open after this stage and the closure assessment is mine.
