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
| 4 | ~~the owner records `codex-developer`, the baseline commit, and non-overlapping ownership~~ → **the owner records the clean baseline commit** | **re-scoped 2026-10-05, see below; still NOT met — owner action** |
| 5 | the metadata/URI/adversarial design has durable independent approval | **met** — [`095-design-review-2026-07-18.md`](095-design-review-2026-07-18.md), by `codex-independent-architecture-security-reviewer`, a role genuinely outside this team |
| 6 | the frozen logout parser/confirmation constants and atomic consumption contract in [`architecture.md`](architecture.md) unchanged | **met** — `git log --since=2026-08-27` on that file is empty |
| 7 | no competing migration claims the next schema number | **met** — see the note below |

## On condition 4 — re-scoped 2026-10-05

**`@nabbisen` ruled on 2026-10-05: "No `codex-developer` now or never. This is
the same mistake made by ex-architect."** The team is defined in
`.git-exclude/roles/`: a human project owner, a high-capability model, and a
mid-capability model that is the implementation and testing agent.
`codex-developer` was a name the former architect invented, and **I repeated it
here instead of checking the roles document** — the same failure as the "admin
UI" and criterion 5 in RFC 134: writing a requirement without checking what it
referred to.

The condition is re-scoped to its one surviving half:

> the owner records the clean baseline commit this work starts from.

**Both withdrawn halves, and why.** Recording the implementation owner is
**vacuous** — there is one implementation role and it is always the
mid-capability model, so naming it per-RFC records nothing. "Non-overlapping
ownership" dates from the two-lane period when a second implementer was
assigned; with one implementation role the collision it guarded cannot occur.

**What survives is not vestigial.** RFC 095's own
[verification.md](verification.md) requires that results from different commits
cannot be assembled into a passing closure package, so the baseline must be
named before coding starts.

**Still an owner action**, and now a smaller one: name the commit.

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
