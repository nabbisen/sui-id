# Developer Handoff — RFC 096-A stage 4d fix: authority on the read paths

## Role and protocol

**Addressee: the mid-capability model (dev team).** Authority on your role is
`.git-exclude/roles/mid-capability-model-operating-instructions.md`.

**On pushing:** `project-instructions-general-common.md:44` authorizes **both**
roles to commit and push. This dispatch asks only that you hand the tree over
uncommitted.

**Clone under `.git-exclude/tmp/clones/`, on `/home`. Hashes with
`python3.14 scripts/hunk-hashes.py --baseline 5a5ec2d`** — the same baseline as
4d, since 4d was not committed.

## What this is

**Stage 4d was returned with one required change.** The review is
`.git-exclude/reviewed/rfc-096-a-stage4d-flights-and-cooldowns-2026-10-08.md`;
read it first. **Nothing else in 4d is reopened.** The eight-row table, the five
rules, `MonotonicClock`, the lock discipline, the bounded map and your
dispatch-not-completion test all stand.

**Your working tree is the starting point** — it was not committed, so there is
nothing to rebase onto.

## The defect

`current_key` is a parameter of `lookup`, consulted **once**, at `:385`, to
decide whether to **retain** a fetched result. **Neither read path consults it
before serving one:**

| Path | Checks |
|---|---|
| fresh hit (`:249-252`) | freshness only |
| join (`:241`) | nothing — joins unconditionally |

So a caller at activation generation 2 can be served a generation-1 entry, or
join a generation-1 flight and consume its result.

**RFC 096 forbids all three readings of that:** `:890` (*"usable only … for the
exact enabled provider/version/activation generation"*), `:891` (*"cache entries
**and single-flight keys** bind all three values"*), `:833` (*"non-authoritative
by durable **version/state comparison**"*).

**The exposure:** an administrator disables and re-enables a provider — changing
issuer, origins or `id_token_algs` — while a JWKS fetch is in the air, and a
caller arriving after the re-enable verifies a token against key material from
the superseded configuration.

## The fix

**Compare before serving.** The slot records the `CacheKey` of its retained
entry **and** of the in-flight fetch.

1. **Fresh hit:** serve only when `current_key()` equals the retained entry's
   key. On mismatch it is a **miss**, and the stale entry is dropped rather than
   left to be re-tested on every lookup.
2. **Join:** a caller whose `current_key()` differs from the in-flight leader's
   key **does not join.** It leads its own flight for its own key. Decide and
   state what the *leader's* slot does with its result in that case — your `:385`
   check already discards a mismatched publish, so the two mechanisms need to
   agree rather than fight.

### Do not key the map on `CacheKey`

It would work and it is worse. Version and activation generation advance over
time, so the map would grow with **activation count** rather than with
configured providers, and would then need pruning. **The bounded-map property
you documented — and reasoned about rather than asserted — is worth more than
the simpler diff.** Comparing is also what `:833` literally asks for.

## Tests

- a fresh generation-1 entry is **not served** at generation 2, and the stale
  entry is dropped;
- the same for a **version** change, not only a generation change;
- a caller at generation 2 **does not consume** a generation-1 flight's result
  and gets its own flight;
- the leader's own publish behaviour in that case matches whatever you decided
  above;
- **and mutate each comparison away, confirming exactly one test fails per
  mutation.** Your rule-1 pass is the right instrument, and it is what found the
  missing dispatch-timing test in the first place.

## Gates

`G01`–`G08`, `G17`, `G18`, `G19`, `G21`. Clean tree, clone on `/home`.

## Package

Hashes from `scripts/hunk-hashes.py` against `5a5ec2d`, gate results, entry
point. **State what the leader does with a result whose key stopped being
current while its own slot now holds a different in-flight key** — that is the
one interaction this fix creates that 4d did not have.
