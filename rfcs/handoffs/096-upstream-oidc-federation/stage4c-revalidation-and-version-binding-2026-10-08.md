# Developer Handoff — RFC 096-A stage 4c: 200/304 revalidation and version binding

## Role and protocol

**Addressee: the mid-capability model (dev team).** Authority on your role is
`.git-exclude/roles/mid-capability-model-operating-instructions.md`.

**On pushing:** `project-instructions-general-common.md:44` authorizes **both**
roles to commit and push. This dispatch asks only that you hand the tree over
uncommitted.

**Clone under `.git-exclude/tmp/clones/`, on `/home`. Hashes with
`python3.14 scripts/hunk-hashes.py --baseline <tip>`.**

## RFC and position

**RFC 096 — Accepted.** Stage 4b landed in `<this commit>`. 4c is the second of
the three cache stages; 4d is the flight and cooldown state table.

**The constraint is unchanged:** nothing here is reachable from
`handlers/federation.rs`, `decode_id_token_claims` is untouched, and
`oidc/jwt.rs` stays EdDSA-only.

## 0. First — the required change from 4b's review

**Treat RFC 096's *"conflicting/duplicate recognized directives"* as one
condition: duplicates.** The strictest directive wins.

**Why, so the reasoning survives:** no pair in
`{no-store, no-cache, must-revalidate, max-age, s-maxage}` is unsatisfiable —
they are all restrictive, so every combination can be honoured. Given
`no-store, max-age=60`, honouring `no-store` retains nothing, which is exactly
what rejecting the response would achieve: **rejection buys no safety and costs
a federated sign-in** against a provider sending a documented belt-and-braces
combination.

**Your reading was well argued and I overruled it because my dispatch created
the ambiguity** — it listed "conflicting" as a condition distinct from
"duplicate" without saying what a conflict could be.

- Remove `ConflictingDirectives` and
  `no_store_conflicts_with_max_age_s_maxage_and_must_revalidate`.
- **Leave a comment saying there are no conflicts in this closed set and why**,
  so nobody re-adds the rule.
- Add a test that `no-store, max-age=60` **parses** and that `no-store`
  governs.

## Scope — RFC 096 `:873-896`

### 1. Every 200 is fully validated, even an unchanged one

*"Every 200 body is bounded, parsed, and semantically validated even when its
ETag equals the retained validator."* A valid 200 **atomically** replaces the
old entry, **only after** full validation. An invalid 200 **never** refreshes
and never extends the old entry's authority beyond its own independent original
expiry.

> **The trap:** an ETag that matches is not a licence to skip validation. Write
> the test where the ETag is unchanged and the body is now invalid, and assert
> the old entry's expiry is untouched rather than renewed.

### 2. What a 304 requires

Accepted **only** for a conditional request naming the **exact retained ETag**
of a validated, cacheable representation at the **exact provider version and
activation generation**. Rejected otherwise — no such body or validator, after
`no-store`, or for a different version or generation. A returned ETag must equal
the requested validator **byte-for-byte**.

### 3. What a 304 does to the retained metadata

| Field | On 304 |
|---|---|
| `Cache-Control` | present **replaces** the retained recognised set; absent **inherits** |
| `Age` | present replaces; **absent becomes zero** |
| `Date` | present replaces; **absent becomes the trusted 304 receipt time** |

*"…so prior freshness age is not silently reused."* **That sentence is the
point of the whole row.** A 304 that omits `Age` must not inherit the old one.

And: `no-store` on a 304 allows the retained body for **that one waiting
operation** and then evicts. `no-cache` or `max-age=0` allow that operation and
make the entry immediately stale. **A 304 never validates a different body and
never bypasses semantic validation already bound to the retained
representation.**

### 4. Version and activation-generation binding

*"An entry is usable only while fresh and for the exact enabled
provider/version/activation generation."* Entries **and** single-flight keys
bind all three, *"so disable/re-enable cannot revive documents fetched in an
older activation even when the trust policy is unchanged."*

**Build the key type in this stage**, even though 4d owns the flights — 4d
needs it and the binding rule belongs with the entry it governs. **Make it
impossible to construct an entry without all three**, the way
`VerifiedIdTokenClaims` is impossible to construct unverified. A tuple that
callers assemble by hand will drift.

### 5. No stale use, ever

No `stale-if-error`, no `stale-while-revalidate`; both ignored and unable to
extend authority. **Clock regression expires the entry** — 4b deferred this
correctly because it had no entry to expire, and **this stage has one**, so
implement it here and say how.

## Not in this stage

The request/cache state table — flights, the 30-second cooldown, the 60-second
forced-refresh window, unknown-`kid` handling. All 4d.

## Tests

Every rule above, and specifically: an unchanged ETag with a now-invalid body;
a 304 whose returned ETag differs by one byte; a 304 after `no-store`; a 304 for
a different provider version, and for a different activation generation; a 304
omitting `Age`, asserting it becomes zero rather than inheriting; a 304 omitting
`Date`, asserting it becomes the receipt time; `no-store` on a 304 permitting
exactly one operation and then evicting.

**Say how an entry proves it carries all three binding values** — if a test
could construct one without them, the type is wrong.

## Gates

`G01`–`G08`, `G17`, `G18`, `G19`, `G21`. Clean tree, clone on `/home`.

## Package

Hashes from `scripts/hunk-hashes.py`, gate results, entry point. **State how
clock regression expires an entry now that there is an entry** — that is the
rule 4b was right to defer and this stage inherits.
