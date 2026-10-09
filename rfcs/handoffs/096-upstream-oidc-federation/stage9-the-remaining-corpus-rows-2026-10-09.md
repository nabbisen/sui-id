# RFC 096-A stage 9 — the remaining corpus rows, and the last stage

**Dispatched.** 2026-10-09 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Baseline.** Read the tip with `git log -1` when you start, hash against it,
name the full SHA.
**Prior stages.** 1–5, 6a (+fix), 6b, 6c (+fix), 7, 8 (+fix) — all landed.
**Twelve of fifteen.**
**This is the last stage of 096-A.** After it, the closure review is mine.
**Open with the owner, untouched here.** `exp` boundary strictness.

## What this stage is

Stage 5 built the hostile-provider corpus for `validation-matrix.md`'s rows that
stages 1–4 implemented, and listed what it could not cover. Two of those
sections are now implemented and have to be collected:

- **Claims** (`validation-matrix.md:129-154`) — sixteen rows. Stage 5 assigned
  these to 096-B1; that was wrong, and the error was **mine**, not theirs. RFC
  096 `:59` gives 096-A *"required ID-token claim validation; the mandatory
  one-time nonce"*, and stages 6a, 6b, 6c and 7 built them.
- **Discovery** (`:64-79`) — five rows plus the well-known derivation vectors,
  built by stage 8.

**It is not pure collection.** I measured three things the matrix explicitly
requires that nothing currently tests — see below.

## Three measured gaps

Each of these is named in the matrix's own prose, and `grep` across
`crates/sui-id/src` and `crates/sui-id/tests` finds nothing exercising it.

**1. Exponential NumericDates.** `:152-153` requires test claims to include
*"string/floating/exponential NumericDates"*. Stage 6b covered string, float,
whole-number float, negative, past-`u64`, and past-`chrono`-range — **not the
exponential literal form**, e.g. `"exp": 1.7e9`. That is a distinct JSON number
*syntax* reaching the same `numeric_date` path, and I expect it is already
refused; prove it rather than assume, and if it is accepted, that is a finding.

**2. A valid multi-byte Unicode `sub`, accepted and preserved exactly.**
`:152-153` requires *"Unicode/control subjects"*. Stage 6a tested the control
character and both length bounds; nothing tests that a legitimate multi-byte
`sub` is **accepted** and returned byte-identical. RFC 096 `:657` says `sub` is
*"preserved exactly"*, and `:693` makes `(provider_id, sub)` the sole identity
key — so a silent normalisation here changes who a user is. Assert the exact
bytes come back, not merely that validation succeeded.

**3. The well-known derivation vectors.** `:74-79` requires root issuer
`https://id.example` to derive `https://id.example/.well-known/openid-configuration`,
path issuer `https://id.example/tenant/a` to derive
`https://id.example/tenant/a/.well-known/openid-configuration`, and the
insert-before-path RFC 8414 form to be **rejected** for the path issuer. Stage
8's package argued this is structurally enforced because `fetch_discovery` only
ever appends — which is true, and **unproven**: nothing tests either issuer
shape end to end.

`fetch_discovery` is in `handlers/federation.rs`, which you may not modify — but
you may **exercise** it. Decide and state how: an e2e test against the live path
with a path issuer is the obvious route, and if the harness cannot express it,
say so rather than substituting a unit test that re-implements the derivation
and proves only itself.

## The layer distinction, which this corpus must carry

Stage 8 established something the corpus has to record. `ValidatedDiscovery::validate`
has exactly one non-test caller, in a file 096-A may not touch. So:

- nine of discovery's eleven metadata rows are **live** after stage 8;
- the two configured-value cross-checks
  (`token_endpoint_auth_methods_supported`, `id_token_signing_alg_values_supported`)
  and the **tightened document bounds** are implemented and **not live**.

**Every row in your table must say which layer proves it** — validation-layer
unit test, or over real TLS on the live path. Stage 5's table already had a
two-column shape for this; keep it and make the distinction explicit rather than
implied. I will be writing the closure assessment from this table, and a row
proven only at the validation layer must not read as though it were proven on
the wire.

## The standard, unchanged from the stages that set it

- **Specific-error assertions**, never `is_err()`, never a wildcard `matches!`
  that would pass for the wrong variant.
- **Both sides of every bound** — 6c's fix established this and stage 8 held it.
  A rejecting test alone leaves the limit free to move.
- **A row you cannot make fail is a finding** about the stage that owns it, not
  something to work around.
- **A row genuinely outside 096-A stays stated, not silently absent.** Stage 5's
  out-of-scope list is the model; correct it where it was wrong. Preflight and
  activation (096-B2), callback and attempt, mapping/MFA/durable effects, and
  the token-response envelope (096-B1) remain outside. The Claims and Discovery
  sections no longer do.
- **A surviving mutant reported honestly beats a contrived test** — stage 7's
  `ct_eq` report is the precedent.

## What to return

A working tree if any test is added, plus a package under
`.git-exclude/review-requests/` with:

1. **The complete table**, extending stage 5's: every Claims and Discovery row,
   the test that covers it, and the layer it is proven at.
2. **The three gaps above**, closed, with mutation evidence that each new test
   is load-bearing.
3. **Any row you could not cover**, and why.
4. **The corrected out-of-scope list.**
5. **Per-hunk SHA-256** against the tip you named; full-content hashes for any
   new file.
6. **Gate evidence** from a throwaway clone with its own `target/`, plus
   `cargo test --workspace` — which has now found a real problem in each of the
   last two stages.
7. **Anything you think is wrong with this dispatch.** Six stages running you
   have corrected something of mine; the three gaps above are measured as of
   today, but the claim that stages 6a–8 cover every *other* Claims and
   Discovery row is my reading of the matrix, not a measurement I completed.

**Do not claim 096-A's closure.** The closure assessment is mine, and after this
lands I will write it against RFC 096 `:21` rather than against the stage list —
which is the check that found the two missing stages in the first place.
