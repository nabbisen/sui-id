# RFC 096-A stage 6b — the time claims: `exp`, `iat`, `nbf`

**Dispatched.** 2026-10-08, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Baseline.** `0e26b05` (stage 6a complete, Level B, 24/24). **Hash against
this commit**, and if the tip has moved when you start, hash against the tip
and say so — the 6a fix's hash report picked up two of my own docs files
because I named a stale baseline.
**Prior stages.** 1, 2, 3a, 3b, 4a, 4b, 4c, 4d, 5, 6a (+fix) — all landed.
**Remaining after this one.** 6c, 7, 8, 9.

## This stage is smaller than it looks — read this before writing anything

I measured `jsonwebtoken-10.3.0/src/validation.rs` while writing stage 5's
closure assessment. **Two of the three rules are already implemented, verbatim,
by the library you are already calling.** Do not rewrite them.

`Validation::new` (`:112-126`) sets `required_spec_claims = {"exp"}`,
`leeway = 60`, `validate_exp = true`, `validate_nbf = false`.

- **`exp` is done.** RFC 096 `:660` says *"Required integer NumericDate; valid
  only while `now < exp + 60s`"*. The library's check (`:291`) is
  `exp - reject_tokens_expiring_in_less_than < now - options.leeway`, which with
  the defaults rejects once `exp < now - 60` — i.e. valid while `now < exp + 60`,
  exactly the rule. `exp` is also already **required**, because it is in
  `required_spec_claims`. **Change nothing about `exp`**, and do not touch
  `leeway` or `reject_tokens_expiring_in_less_than`. Add a test proving the
  boundary holds, and a comment recording that the library owns it and why.
- **`nbf` is one flag.** RFC 096 `:662` says *"Optional integer; `nbf <= now + 60s`"*.
  The library's check (`:296`) is `nbf > now + options.leeway` → reject, which is
  that rule. It is switched off by default. Set `validate_nbf = true` in
  `validation_for`. Keep `nbf` **out** of `required_spec_claims` — the RFC says
  optional, and adding it there would make it required.
- **`iat` is entirely ours.** The library has no `iat` validation at all.

Verify each of these yourself rather than taking my word for it — the file is
in the vendored registry source and the line numbers are above. If any of it has
changed, that is a finding and I want it before the implementation, not after.

## `iat` — the only rule to write

RFC 096 `:661`: *"Required integer; `created_at - 60s <= iat <= now + 60s`"*,
and `:676`: *"`iat` is additionally bound to this attempt so an otherwise valid
old token cannot be substituted."*

**`created_at` is a parameter.** 096-A performs no durable mutation (`:63-66`),
so the attempt's creation time arrives as an argument, exactly as
`expected_issuer` and `expected_client_id` do in 6a. The durable attempt record
that supplies it is 096-B1's.

Note what the lower bound is for: it is the substitution defence. A token minted
before this attempt started is refused even when its signature, issuer,
audience and expiry are all perfect. That is the rule's whole purpose, so the
test for it should say so in its name.

## NumericDate discipline

RFC 096 `:673-676`: NumericDate values must be **JSON integers** in a range the
application clock can represent. *"floats, strings, overflow, and duplicate
claims fail."* Each of these is its own refusal, by its own name:

- a float (`1700000000.5`), including one with a zero fraction (`1700000000.0`);
- a string (`"1700000000"`), which is the common real-world provider bug;
- a negative value, and one past the clock's representable range;
- `true`/`null`/an array/an object where an integer is required.

**Duplicates are already handled** — stage 6a's `first_duplicate_member` covers
every top-level member, so a repeated `iat` is refused by name before this
layer. Do not add a second check; add a test asserting the existing one covers
`iat`, and cite it.

The 60-second skew is **fixed and symmetric only where the RFC states it**
(`:675`). It is not provider-controlled and not configurable. Write it as a
named constant, once.

## What `exp` still needs from you

The library owns the rule, but the library's `exp` failure arrives as
`SignatureInvalid` — the same collapse that stage 6a's fix just corrected for
duplicates. **Check whether an expired token currently reports as
`SignatureInvalid`**, and if it does, give it its own `VerificationError`
variant the same way. A token whose signature verified and whose `exp` has
passed must not be reported as a signature failure; that is the identical
defect, one claim over.

I have not measured this one — it is the first thing to find out, and if it
turns out `jsonwebtoken` already distinguishes it, say so and change nothing.

## Where it goes

Tests beside, not inside (RFC 137), with the `#[path]` pattern. `identity_claims.rs`
is the natural home if the time claims join the same validation entry point;
a `time_claims.rs` sibling is equally defensible if you would rather keep the
parameter lists apart. **Say which and why in one line** — 6c adds a third
group, so whichever you choose is the shape all three live in.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The row-to-test table**, 6a's format, covering the three claims and every
   NumericDate rejection.
2. **Your reading of `validation.rs`** — confirming or correcting the four
   measured facts above. If the `exp` boundary is exactly as I state, prove it
   with a test at `now == exp + 60` and `now == exp + 61`, not with a citation.
3. **The `exp` error-name finding**, whichever way it falls.
4. **Mutation evidence**, per claim, picking the mutation that isolates the rule
   — 6a's fix showed the value of that: the adjacent-only mutation proved its
   fifth test earned its place in a way gating the whole scan could not.
5. **Per-hunk SHA-256** against the baseline you name.
6. **Gate evidence** from a throwaway clone under `.git-exclude/tmp/clones/`
   with its own `target/`.
7. **Anything you think is wrong with this dispatch.** Two of the last three
   stages were improved by you pushing back; the `exp`/`nbf` facts above are
   measurements I made days ago and have not re-checked.

**Not 096-A's closure.** Token substitution needs stage 7's nonce rule, and
discovery is stage 8. The closure assessment stays mine.
