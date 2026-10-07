# Developer Handoff — RFC 096-A stage 4d: flights, cooldowns, and forced refresh

## Role and protocol

**Addressee: the mid-capability model (dev team).** Authority on your role is
`.git-exclude/roles/mid-capability-model-operating-instructions.md`.

**On pushing:** `project-instructions-general-common.md:44` authorizes **both**
roles to commit and push. This dispatch asks only that you hand the tree over
uncommitted.

**Clone under `.git-exclude/tmp/clones/`, on `/home`. Hashes with
`python3.14 scripts/hunk-hashes.py --baseline <tip>`.**

## RFC and position

**RFC 096 — Accepted.** Stage 4c landed in `<this commit>`. **4d is the last
cache stage and the hardest**: it is the only one with concurrency.

**The constraint is unchanged:** nothing here is reachable from
`handlers/federation.rs`, `decode_id_token_claims` is untouched, and
`oidc/jwt.rs` stays EdDSA-only.

**Use 4c's `CacheKey`.** The RFC is explicit that *"cache entries **and
single-flight keys** bind all three values"* — provider, version, activation
generation — *"so disable/re-enable cannot revive documents fetched in an older
activation."* A flight keyed on provider alone would reintroduce exactly what
4c's constructor prevents.

## Scope — RFC 096 `:897-924`

### The closed state table

Implement it as written. **Eight rows, and the order matters.**

| Situation | Rule |
|---|---|
| Ordinary lookup, fresh entry | return it; **no network** |
| Ordinary lookup, stale/miss, active 30 s failure cooldown | **fail; no network** |
| Ordinary lookup, stale/miss, active flight | **join that one flight** |
| Ordinary lookup otherwise | become **one** conditional/unconditional flight leader |
| Unknown `kid`, active cooldown | fail; **do not consume forced-refresh budget** |
| Unknown `kid`, active ordinary **or** forced flight | join it; **that result is the sole refresh opportunity for this callback** |
| Unknown `kid`, no flight, forced dispatch within prior 60 s | **fail; no network** |
| Unknown `kid` otherwise | mark dispatch time, become **one** forced JWKS flight leader, **bypassing fresh-document reuse** |

### Five rules that are easy to get subtly wrong

1. **The 60-second forced window begins when the request is *dispatched*** —
   *"whether it returns 200, 304, or failure."* **Not on completion.** A slow
   request must not extend the budget; if it did, an attacker could hold the
   window open.
2. **A failed flight starts the 30-second cooldown, and joined callers observe
   the same result.** One failure, one cooldown, one outcome for everyone
   waiting — not a cooldown per caller.
3. **A forced 304 proves the key set unchanged, so the unknown `kid` still
   fails — without a second request.** A 304 is a successful refresh *and* a
   definitive answer that the key is absent. Do not retry.
4. **Publication occurs only if the provider remains enabled at the same
   version.** A flight that completes after a disable or a version change must
   discard its result rather than publish it.
5. **Maps are bounded by configured providers and document types — *never* by
   attacker `kid` values.** **This is the security-critical line in the whole
   stage.** A map keyed on `kid` is an unbounded attacker-controlled allocation.
   State in the package what the key type is and why it cannot grow with
   traffic.

### Rotation, RFC 096 `:920-924`

*"Once a refreshed valid JWKS omits a key, that key is not retained as a
fallback. Emergency removal therefore fails closed after refresh."* No
grace-retention of a vanished key, for any reason. Operators may disable the
provider immediately; that is the escape hatch, and it is theirs, not the
cache's.

## Tests

Every row of the table, and each of the five rules above. Specifically:

- two concurrent ordinary lookups produce **one** network request, and both see
  the same outcome;
- a failed flight puts the *provider/document* into cooldown, and a second
  caller during it fails **without** a request;
- an unknown `kid` during cooldown fails **and the forced budget is still
  intact afterwards** — assert the budget, not just the failure;
- a forced flight dispatched, then a second unknown `kid` 59 s later fails with
  no request, and at 61 s is allowed;
- a forced flight that returns **304** fails the unknown `kid` and makes **no**
  second request;
- a flight completing after the provider is disabled, and after a version
  change, **does not publish**;
- a refreshed JWKS omitting a previously present `kid` makes that `kid` fail.

**Drive time through the injected clock, and concurrency deterministically.**
A test that races real threads will pass on your machine and fail in CI at 3 a.m.
If you need a scheduling seam to make "two callers, one flight" deterministic,
introduce one and say so — that is a design choice worth reviewing, not a detail.

## Gates

`G01`–`G08`, `G17`, `G18`, `G19`, `G21`. Clean tree, clone on `/home`.

## Package

Hashes from `scripts/hunk-hashes.py`, gate results, entry point. **Three things
stated explicitly:**

1. the **key type** of every map, and why none can grow with attacker input;
2. how **"one flight"** is enforced, and what a joined caller observes;
3. whether you introduced a scheduling seam for the tests, and if so where.
