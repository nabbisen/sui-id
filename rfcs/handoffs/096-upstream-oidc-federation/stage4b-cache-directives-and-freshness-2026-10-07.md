# Developer Handoff — RFC 096-A stage 4b: cache directives, age, and freshness

## Role and protocol

**Addressee: the mid-capability model (dev team).** Authority on your role is
`.git-exclude/roles/mid-capability-model-operating-instructions.md`.

**On pushing:** `project-instructions-general-common.md:44` authorizes **both**
roles to commit and push. This dispatch asks only that you hand the tree over
uncommitted.

**Clone under `.git-exclude/tmp/clones/`, on `/home`. Hashes with
`python3.14 scripts/hunk-hashes.py --baseline <tip>`** — it exists now, it
landed in `606704a`.

## RFC and position

**RFC 096 — Accepted.** Stage 4a landed in `8a697b1`.

**Stage 4 splits again, and this is why.** I called `:823-924` one stage, then
read it line by line. It is **three separable concerns**, and one dispatch
covering all of them would be the largest of this RFC and effectively
unreviewable:

| Stage | Concern |
|---|---|
| **4b — this one** | the closed directive set, `Age`/`Date`/`ETag` validation, and freshness arithmetic. **Pure functions. No state, no network, no concurrency.** |
| 4c | 200/304 revalidation semantics, and binding to provider version and activation generation |
| 4d | the request/cache state table: single flight, the 30-second cooldown, the 60-second forced-refresh window, unknown-`kid` handling |

**096-A is nine stages, not six.** I would rather say that now than discover it
at 4d.

**The constraint is unchanged:** nothing here is reachable from
`handlers/federation.rs`, `decode_id_token_claims` is untouched, and
`oidc/jwt.rs` stays EdDSA-only.

## Scope — RFC 096 `:841-872`

### 1. The closed directive set

Recognised: `no-store`, `no-cache`, `must-revalidate`, `max-age`, `s-maxage`.
**Case-insensitive.** Multiple `Cache-Control` field lines combine as an HTTP
list — **and `Cache-Control` is the only field that may.**

**Reject the response** on: a recognised directive occurring more than once,
conflicting recognised directives, **quoted or non-canonical decimal
delta-seconds**, overflow, invalid list syntax, or obsolete line folding.
**Ignore** unknown directives, bounded.

| Directive | Behaviour |
|---|---|
| `s-maxage` | **parsed and then ignored** — this is a private cache. Parse it so a malformed one is still a rejection |
| `must-revalidate` | **recorded, no behaviour** — stale use is always forbidden, so it adds nothing. Record it anyway so the retained set is faithful |
| `no-store` | the validated 200 may be used **for the current operation only**; retain **no** entry and **no** ETag |
| `no-cache` | retention allowed, **zero** reusable freshness |
| `max-age=0` | zero freshness |
| positive `max-age` | honoured exactly, **clamped only at the per-cache maximum**. **Never raised to a minimum** |
| absent `max-age` | the table default, **unless `no-cache` is present** |

| Cache | Default freshness | Maximum |
|---|---|---|
| Discovery | 1 hour | 24 hours |
| JWKS | 15 minutes | 6 hours |

### 2. `Age`, `Date`, `ETag`, `Content-Type`

**Exactly zero or one of each.** Duplicates reject.

- `Age` — canonical decimal seconds, bounded by `u32::MAX`.
- `Date` — IMF-fixdate. **Reject if later than the trusted wall clock + 60 s.**
- `ETag` — one strong **or** weak entity-tag matching the HTTP grammar, **at
  most 256 visible/quoted bytes**. Duplicate, malformed, control characters, or
  oversize reject the response.

### 3. Freshness arithmetic — the part worth testing hardest

```
initial_current_age = max( Age (0 if absent),
                           max(0, trusted_now - Date) (0 if absent) )
current_age         = initial_current_age + monotonic resident time
fresh               = lifetime > current_age
```

> **Equality is stale.** `lifetime == current_age` is **not** fresh. Write that
> test first; it is the boundary an implementation gets wrong by default.

**Clock regression expires the entry.** Use a monotonic source for resident
time — the wall clock appears only in the `Date` comparison — so that a wall
clock moving backwards cannot extend an entry's life.

### Not in this stage

No cache *storage*, no 200/304 handling, no flights or cooldowns, no network.
**Pure functions over parsed header values and a clock**, so every rule above
is testable without a server.

## Tests

Every rejection above, each at the boundary where it has one: `Age` at
`u32::MAX` and one past it; an ETag at 256 bytes and 257; `Date` at
trusted-now + 60 s and + 61 s; `max-age` at the per-cache maximum and one past
it, showing it **clamps rather than rejects**; a `max-age` below the default,
showing it is **not** raised.

And: `lifetime == current_age` is **stale**; a wall clock moving backwards does
not extend life; `s-maxage` is accepted and has no effect, while a malformed
`s-maxage` still rejects; `must-revalidate` is recorded and changes nothing;
two `Cache-Control` lines combine, while two `Age` lines reject.

**Inject the clock.** A test that sleeps is a test that will be flaky at 3 a.m.
in CI.

## Gates

`G01`–`G08`, `G17`, `G18`, `G19`, `G21`. Clean tree, clone on `/home`.

## Package

Hashes from `scripts/hunk-hashes.py`, gate results, entry point. **State which
clock abstraction you introduced and where it is injected** — that is the one
design choice this stage makes that 4c and 4d will both have to live with.
