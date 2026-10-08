# Developer Handoff — RFC 096-A stage 5: the hostile-provider corpus

## Role and protocol

**Addressee: the mid-capability model (dev team).** Authority on your role is
`.git-exclude/roles/mid-capability-model-operating-instructions.md`.

**On pushing:** `project-instructions-general-common.md:44` authorizes **both**
roles to commit and push. This dispatch asks only that you hand the tree over
uncommitted.

**Clone under `.git-exclude/tmp/clones/`, on `/home`. Hashes with
`python3.14 scripts/hunk-hashes.py --baseline <tip>`.**

## RFC and position

**RFC 096 — Accepted.** The cache work is complete: 4b, 4c, 4d and 4d's fix all
landed. **Stage 5 is the last stage of 096-A.**

**The harness exists.** `r096_a_harness.rs` and
`egress::build_federation_client_for_tests` landed in `7c068f8` — a raw-byte TLS
fixture reached by a client built from the shared production constructor with
only the resolver and one extra trusted root replaced. **Two named differences,
and G19 covers the test tree.** Do not build a second harness.

## What this stage is, and what it is not

**It drives every negative row of
[`validation-matrix.md`](validation-matrix.md) through the code stages 1–4
built, and proves each one is refused.**

**It is not new validation logic.** If a row cannot be made to fail, that is a
finding about stages 1–4, not a licence to add a check here. **Report it; do not
fix it in the corpus.** A corpus that quietly patches the thing it is testing
proves nothing.

**And it is still not production.** `handlers/federation.rs` remains untouched;
096-B1 routes live traffic, and it needs RFC 094 M2a.

## Scope — every negative row, named by its stage

Work through `validation-matrix.md` row by row. For each, the corpus needs a
hostile fixture and an assertion naming **the specific refusal**, not merely
that something failed:

| Area | Stage that refuses it |
|---|---|
| ID-token `alg`: `none`, `HS*`, unknown, empty configured set | 1 (config) and 4a (membership) |
| compact structure: wrong segment count, empty segment, JWE, JSON serialization, padding, oversize | 2 |
| header: missing `kid`, oversize `kid`, `jku`/`x5u`/`jwk`/`x5c`/`crit`/`b64`, `cty`, bad `typ`, duplicate member | 2 |
| JWKS: oversize, too deep, too many keys, duplicate member, duplicate `kid` | 3a |
| key selection: `kid` miss, private-key member, `use`/`key_ops`, family, curve, RSA size and exponent, ambiguity | 3b |
| signature: altered byte, wrong key, `jwks_uri` absent | 4a |
| cache: directive and header rejections, superseded version or generation, no stale use | 4b, 4c, 4d |
| transport: hostile endpoints, oversized responses, redirect | RFC 134's resolver and bounds, plus 3a |
| **rotating keys** | 4c/4d — *"once a refreshed valid JWKS omits a key, that key is not retained as a fallback"* |

**RFC 096's closure prerequisite names these explicitly** — *"Substitution,
missing or mismatched nonce, issuer/audience/time errors, algorithm confusion,
hostile endpoints, oversized responses and rotating keys are each handled as
designed."* **Nonce and issuer/audience/time belong to 096-B1's claim
validation, not here.** Say so in the package rather than leaving a gap that
looks like an omission: this stage covers the rows 096-A's stages built.

## Two things to get right

1. **Assert the specific error, not `is_err()`.** Every stage built one variant
   per rule precisely so a corpus could tell them apart. A test that accepts any
   failure would pass if the code refused for the wrong reason — which is how
   algorithm-confusion bugs hide.
2. **One fixture per row, named after the row.** When a row later regresses, the
   failing test name should say which rule broke without anyone reading the body.

## Gates

`G01`–`G08`, `G17`, `G18`, **`G19`** (it covers the test tree, and this stage is
mostly test tree), `G21`. Clean tree, clone on `/home`.

## Package

Hashes from `scripts/hunk-hashes.py`, gate results, entry point. **And three
statements:**

1. **every matrix row, with the test that covers it** — a table, so the mapping
   is checkable rather than asserted;
2. **any row you could not make fail**, which is a finding about stages 1–4;
3. **any row you judged out of 096-A's scope** — nonce, issuer, audience, time
   — stated as such rather than silently absent.

**When this lands, 096-A's closure prerequisites come into view.** Do not claim
them; the closure review is mine. But the corpus is the evidence they rest on,
so what you cannot prove here is what I will have to tell the owner.
