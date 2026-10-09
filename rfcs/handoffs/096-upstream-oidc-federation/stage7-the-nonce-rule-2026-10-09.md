# RFC 096-A stage 7 — the nonce rule

**Dispatched.** 2026-10-09 JST, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Baseline.** Read the tip with `git log -1` when you start, hash against it,
name the full SHA.
**Prior stages.** 1, 2, 3a, 3b, 4a, 4b, 4c, 4d, 5, 6a (+fix), 6b, 6c (+fix) —
all landed. **Ten of fifteen.**
**Remaining after this one.** 8 (discovery), 9 (the corpus rows).
**Open with the owner, touching nothing here.** `exp` boundary strictness.

## The rule, and the line that splits it

RFC 096 `:663`:

> `nonce` | Required string; digest compared in constant time to the attempt's nonce digest

And `:65-66`, which is why this is 096-A's and not 096-B1's:

> It delivers the *nonce validation rule*; the durable attempt state that makes
> a nonce genuinely one-time is 096-B1, because it is a mutation.

So: **the expected digest is a parameter.** You compare against it; you do not
load it, store it, or mark it consumed. One-time-ness is 096-B1's.

## Measured for you — three things already in the tree

I checked these rather than describing them, and the point of each is that you
should **not** write a new one.

**1. The algorithm is SHA-256, and the RFC settles it by naming the column.**
`:579` gives the `federation_login_attempt` schema with `nonce_sha256` among
`state_sha256` and `browser_binding_sha256`. No other digest is in play.

**2. `sha256_hex` already exists and is public.**
`sui_id_core::oidc::tokens::sha256_hex(&str) -> String` at `tokens.rs:209` —
lowercase hex, with a known-vector test (`tokens/tests.rs:22`). **Use it.**

There is already a **private duplicate** of the same function at
`crates/sui-id/src/http/handlers/dynamic_register.rs:327`, so the tree holds two
implementations of one digest. Do not make it three. Fixing that duplication is
not this stage's job — note it in your package and I will schedule it
separately; I would rather not widen a security-path stage to tidy an unrelated
handler.

**3. The constant-time idiom is established, with five call sites.**
`subtle::ConstantTimeEq`, already a direct dependency of `crates/sui-id`
(`Cargo.toml:94`, workspace pin `2.6`). The existing shape is
`a.as_bytes().ct_eq(b.as_bytes()).into()`, used at `federation_state.rs:46-47`,
`csrf.rs:38,102`, `handlers/metrics.rs:107,144`, and
`handlers/federation.rs:284-288` — the last of which already does exactly this
for the `state` parameter on the federation callback. **Match that shape.**

No new dependency is needed. If you think one is, say why before adding it.

## What to be careful about

**`ct_eq` is constant-time in the contents, not in the length.** On unequal
lengths it returns false, and the length comparison itself is not hidden. For
two fixed-length 64-character hex digests that is a non-issue — but it stops
being one if a malformed or truncated expected digest can reach you. **Decide
and state** whether you reject a non-64-hex expected digest up front as a
precondition, or compare regardless. I lean to rejecting it with its own error:
a caller handing you a short digest has a bug, and failing loudly beats a
comparison that is quietly weaker than it looks.

**Compare digests, not nonces.** Hash the token's `nonce` claim and compare the
result to the supplied digest. Do not accept a plaintext expected nonce — the
RFC's schema stores a digest, and taking plaintext would invite a caller to pass
the raw value.

**The nonce claim itself still needs its shape checked** before hashing:
required, and a string (`:663` says *"Required string"*). An absent `nonce`, or
one that is a number or object, is a refusal by its own name — not a digest
mismatch. These are different faults and a reader of the error should be able to
tell which happened.

**Duplicate `nonce` is already handled.** Stage 6a's `first_duplicate_member`
covers every top-level member before `decode`. Do not add a second check — add a
test asserting the existing one covers `nonce`, and cite it.

**The capability must not carry the nonce.** Stage 6c's `IdentityCapability`
satisfies RFC 096 `:687-689` by holding no nonce at all, and its
`the_capability_holds_no_trace_of_the_tokens_nonce` test proves it. Validate the
nonce and discard it; nothing you add may put it into the capability or into any
`Debug` output.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The row-to-test table**: absent, wrong type, digest mismatch, digest match,
   and whatever precondition you choose for a malformed expected digest. Both
   sides of the comparison, not just the failing one — 6c's fix is the standard
   now.
2. **The constant-time argument**, naming which primitive you used, where the
   existing call sites are, and your decision on the length question above.
3. **Mutation evidence**, including one that replaces `ct_eq` with `==`. That
   mutation will *not* fail any test — equality is still equality — so say so
   plainly and explain what does protect the property. **This is the one place
   in this stage where a surviving mutant is the honest answer**, and I would
   rather have that stated than see a test contrived to appear to catch it.
4. **Per-hunk SHA-256** against the tip you named.
5. **Gate evidence** from a throwaway clone with its own `target/`.
6. **Anything you think is wrong with this dispatch.** Four stages running you
   have corrected something of mine, and in the last two it was my arithmetic
   and my test-count. The three measurements above are fresh as of today, but
   re-check them.

**Not 096-A's closure.** Token substitution closes when this lands, since
`aud`, `iss` and the nonce are then all bound — but discovery (8) and the corpus
rows (9) remain, and the closure assessment is mine to write.
