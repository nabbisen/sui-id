# RFC 095 M3 — entry-gate assessment

**Date:** 2026-10-03
**Status: not dispatched, and cannot be.** Five of the six entry-gate conditions
in [`README.md`](README.md) §"Entry gate" are met. **The sixth is an owner
action.** Nothing here is work for the dev team yet.
**Why now:** M2a closed 2026-10-03, which was RFC 095's only *technical* blocker.

## The gate, measured

| # | Condition | State |
|---|---|---|
| 1 | RFC 095 in `rfcs/accepted/` with complete approval metadata | **met** — G11 passes on it |
| 2 | RFC 094 **M2a** Implemented, C15's transaction having observed rollback **and exactly-once** evidence | **met** — M2a closed 2026-10-03; `C15` appears in **both** of G20's registries |
| 3 | RFC 093's current clean-tree matrix passes | **met** — Level B on `842b75b`, 26 jobs, 0 skipped |
| 4 | **the owner records `codex-developer`, the full clean baseline commit, and non-overlapping ownership of the files** | **NOT met — owner action** |
| 5 | the metadata/URI/adversarial design has durable independent approval | **met** — [`095-design-review-2026-07-18.md`](095-design-review-2026-07-18.md), by `codex-independent-architecture-security-reviewer`, a role genuinely outside this team |
| 6 | the frozen logout parser/confirmation constants and atomic consumption contract in [`architecture.md`](architecture.md) unchanged | **met** — `git log --since=2026-08-27` on that file is empty |
| 7 | no competing migration claims the next schema number | **met** — see the note below |

## On condition 4, which is the only blocker

**I think it is largely vestigial, and I am not treating it as if I had decided
that.** It dates from the two-lane period, when a second implementer had been
assigned and "non-overlapping ownership" meant two implementers not colliding in
the same files. There is one implementation role now, so the collision the
clause guards against cannot occur.

What survives and is still worth recording is the **baseline commit**: RFC 095's
own verification doc requires that "results from different commits cannot be
assembled into a passing closure package", so the baseline this work starts from
should be named before it starts.

**Three ways to settle it, and the choice is `@nabbisen`'s:**

1. **Record it as written** — name `codex-developer`, name the baseline commit,
   and state that ownership is trivially non-overlapping with one implementer.
   Cheapest, keeps the gate's text honest.
2. **Record the baseline only** and note the ownership clause as spent, the way
   the dated-window clauses were withdrawn on 2026-10-02.
3. **Say the gate is met** and let me record that, with the reasoning.

My recommendation is **(1)**: it costs a sentence, it leaves no clause
half-applied, and the baseline is genuinely useful.

## Two things to know before the work starts

**The migration number moved.** RFC 095's `Touches` names "schema migration 0039
or its next available equivalent". `0039` is long gone, and **`0045` was taken
today** by RFC 134 D3 (`0045_federation_provider_allowed_origins.sql`). The next
free number is **`0046`**. Condition 7 is met, but only as of today — RFC 134's
remaining steps do not add migrations, so it should stay free.

**C15's work did not pre-empt RFC 095.** M2a made the C15 *transaction*
validate-first and atomic. RFC 095's scope is the **metadata contract** on top of
it, which is almost entirely unbuilt: `metadata-validation.md` specifies seven
areas — envelope and member policy, supported fields, the redirect corpus, the
name/scope corpus, the authentication/grant matrix, browser presentation, and
response/error assertions. Today `dynamic_register.rs` validates redirect URIs
via `admin::clients::validate_redirect_uri` and four application-identity URIs
via a local `validated_uri` helper, and little else. **The overlap is the
transaction, not the validation**, so the bulk of RFC 095 remains.

## What happens when condition 4 clears

I write the dispatch. It will be staged rather than one package — the metadata
contract, then the authentication/grant matrix, then logout and CORS — because
`metadata-validation.md` is a corpus specification and a single package against
all seven areas would be unreviewable.
