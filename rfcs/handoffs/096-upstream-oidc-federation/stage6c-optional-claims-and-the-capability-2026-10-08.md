# RFC 096-A stage 6c — the bounded optional claims, and the construction capability

**Dispatched.** 2026-10-08, by the architect.
**RFC.** 096 — Upstream OIDC Federation Validation. **Status: Accepted.**
**Baseline.** Read the tip yourself with `git log -1` when you start, hash
against it, and name the full SHA — as you did in 6b. Do not trust a short hash
written in a dispatch; two of mine have now been stale or nonexistent.
**Prior stages.** 1, 2, 3a, 3b, 4a, 4b, 4c, 4d, 5, 6a (+fix), 6b — all landed.
**Remaining after this one.** 7 (the nonce rule), 8 (discovery), 9 (the corpus
rows stage 5 could not cover).
**Open, not blocking you.** RFC 096 `:660`'s `exp` strictness is with the owner
(`exp-boundary-strictness-2026-10-08.md`). It touches nothing here.

## Two halves, and the second is the one with teeth

### Half one — the bounded optional claims

RFC 096 `:663-672`, verbatim. Every one of these is **optional**; a present-but-
invalid value is a **rejection**, never a silent ignore (`:679-681`).

| Claim | Rule |
|---|---|
| `email` | Optional valid mailbox-shaped UTF-8 string, at most 254 bytes; metadata only |
| `email_verified` | Optional boolean; email is provisioning-authoritative only when exactly `true` |
| `preferred_username` | Optional string, at most 128 scalars/512 UTF-8 bytes, no control or bidi override/isolate; a derivation hint only |
| `name` | Optional string, at most 256 scalars/1,024 UTF-8 bytes, no control or bidi override/isolate; display hint only |
| `amr` | Optional array of at most 16 unique visible-ASCII strings, each 1–64 bytes; ignored for authority, not persisted |
| `acr` | Optional visible-ASCII string, 1–256 bytes; ignored for authority, not persisted |
| `auth_time` | Optional integer NumericDate in the representable clock range; ignored for authority, not persisted |
| `at_hash` | Optional visible-ASCII string, 1–256 bytes; ignored because the access token carries no identity or API authority |

Points that are easy to get wrong, so state your reading of each in the package:

- **Two different limits per string claim.** `preferred_username` is *128
  scalars* **and** *512 UTF-8 bytes*; `name` is *256* and *1,024*. Both bounds
  apply — a 200-scalar name of 3-byte characters passes the byte bound and
  fails the scalar one. Use `chars().count()` and `len()`, and test each bound
  independently so a test failure tells you which one fired.
- **"no control or bidi override/isolate"** is more than `char::is_control`.
  The bidi overrides and isolates are `U+202A`–`U+202E` and `U+2066`–`U+2069`,
  and they are not control characters by Rust's definition. Name them as
  constants and cite this line.
- **`amr` uniqueness** is per-array, like `aud`'s in 6a.
- **`auth_time` reuses 6b's `numeric_date`.** Do not write a second NumericDate
  parser; if its signature does not fit, say so rather than copying it.
- **`email_verified` must be exactly `true`** to carry authority. A string
  `"true"`, a `1`, or anything else non-boolean is a rejection, not a falsy
  value. This is the claim most likely to be sent wrong by a real provider.
- **`at_hash` is validated and then deliberately ignored.** RFC 096 `:682-684`
  explains why. Record that reasoning where someone who later thinks the
  validation is dead code will read it.

### Half two — the construction capability

RFC 096 `:687-689`:

> Successful validation returns a non-cloneable construction capability holding
> provider ID/version/activation generation, exact `sub`, verified email state,
> bounded display hints, and validation time. It contains no raw token, nonce,
> or upstream access token.

**This is the stage that makes that true**, and it is why 6a's review insisted
on deleting `raw_payload` rather than letting it ride until now.

- **Non-cloneable.** No `#[derive(Clone)]`, and say in the package what stops a
  caller cloning it by hand.
- **Think about `Debug`.** `VerifiedIdTokenClaims` derives `Debug` and holds the
  whole payload `Value`, nonce included, so `{:?}` prints it. The capability
  must not have that property. Decide whether it derives `Debug` at all, or
  implements it by hand to redact — and justify the choice. A capability whose
  `Debug` dumps a nonce into a log is the failure this clause exists to prevent.
- **`provider ID/version/activation generation`** is the `CacheKey` triple from
  stage 4c. Take it as a parameter; 096-A performs no durable mutation.
- **`validation time`** is a parameter too, not a clock read inside the
  constructor — otherwise it cannot be tested deterministically, and 6b already
  found what an in-function clock read does to a boundary test.
- **Sealed like its three predecessors**, with a `compile_fail` fixture:
  `VerifiedIdTokenClaims` (4a), `RetainedEntry` (4c),
  `RequiredIdentityClaims` (6a).
- **"no raw token, nonce, or upstream access token"** is an assertion you should
  be able to *demonstrate*, not just honour. Propose how — a test that the type
  holds no `String` capable of carrying the token, a `Debug` output assertion, or
  something better you think of.

## Scope boundary

The nonce is **stage 7**, not this one. The capability carries no nonce by this
clause, so the two do not collide: 7 validates the nonce and discards it.

## What to return

A working tree, plus a package under `.git-exclude/review-requests/` with:

1. **The row-to-test table**, your established format, every claim and every
   rejection, with the two string claims' bounds tested separately.
2. **Your reading of the eight rules**, especially the scalar-versus-byte pairs
   and the bidi set.
3. **The capability's design**, with the `Debug` decision and its reasoning, and
   how you demonstrate the "no raw token, nonce, or access token" clause.
4. **Mutation evidence**, picking the mutation that isolates each rule. 6b set
   the bar here: a mutation that kills nothing is a finding about the tests. If
   one of yours kills nothing, say so and fix the test, as you did for the
   `chrono` range.
5. **Per-hunk SHA-256** against the tip you named.
6. **Gate evidence** from a throwaway clone with its own `target/`.
7. **Anything you think is wrong with this dispatch.** Three stages running you
   have corrected one of my assertions, and in 6b both of them were about `exp`.
   Assume the same rate here.

**Not 096-A's closure.** Substitution still needs stage 7; discovery is 8. The
closure assessment is mine.
