# Closure review — RFC 131

**Date:** 2026-10-02
**Reviewed by.** The architect (high-capability model, requirements-architect role).
**Approved by.** `@nabbisen`, 2026-10-02: *"Approved. Write it."*
**Commit cited.** `fc056df` — Level B green on it: CI run `36878881472`, 24 of 24 jobs.

**A note on the order, because it was unusual and should not become a habit.**
`@nabbisen` approved this closure **before** the evidence was written — he had
just approved the amendment that made the RFC closable, and the batch-1 review had
already set out why the D5 clause was the only obstacle. I verified the three
remaining clauses *before* acting on that approval, and **would have stopped and
reported rather than closed** had one failed. They hold; what follows is the
evidence, written after the approval but not shaped by it.

**Why this RFC closes alone.** It was withdrawn from batch 1 on 2026-10-02 because
its own `Closure prerequisites` required D5 — "a milestone cannot be closed over an
RFC that has not been" — which D5's implementation had measured to be unbuildable
against `ROADMAP.md`'s milestone table. An RFC may not require, as a condition of
its own closure, something it has itself established cannot be built. The clause
was removed on `@nabbisen`'s approval, the reason recorded in place, and the
property's unenforceability remains documented in D5 and in ROADMAP.

## The three remaining clauses

| Clause | Finding |
|---|---|
| There is **exactly one answer** to "what must pass before this claim", the same for all three claims that make it | Level B is defined as *every gate in `contracts/gate-inputs.toml`'s `[gates]` table* and is deliberately not written down as a separate list, so it cannot drift from the table it mirrors. `docs/src/contributing/release-process.md:102` states it in those terms |
| …and **no document states a weaker one** | `scripts/check-verification-commands.py` reports *"no document restates a gate's command"* across `docs/src/**` and `.github/**`, and it is wired into A3.4 as condition 10 (`scripts/check-gate-inputs.sh:557`) — so this is enforced on every run, not asserted here |
| **A tagged version is either on the registry or recorded as abandoned, and the discrepancy is detected without anyone remembering to look** (D7) | `scripts/check-published-versions.py`, a required release step. Its tests defend the clause directly, and I read them rather than counting them: `test_one_crate_behind_is_exit_one` (the discrepancy **is** detected), `test_an_abandoned_tag_is_exit_zero_without_querying_the_registry`, `test_a_version_merely_named_as_where_content_shipped_is_not_abandoned` (guards the false positive), `test_a_registry_error_is_exit_two` (fails loudly rather than passing on error), `test_semver_key_orders_numerically_not_lexicographically`, and — the strongest — **`test_the_real_changelog_marks_exactly_the_four_known_gaps`**, which pins 0.76.10, 0.76.11, 0.76.12 and 0.78.0 against the real `CHANGELOG.md` rather than a fixture |
| `release-process.md` cannot drift from the Gate Matrix without a gate failing | Same checker as above; the document is in its scan set and currently clean |

**All three met.**

## One clause met in letter, and worth saying how

*"…without anyone remembering to look."* D7's first draft met this with a weekly
cron. That was corrected the same day on finding
`.github/workflows/fuzz.yml:3-5` — a cron here *"failed eight consecutive weeks
unnoticed"* — so the check is now a **required release step** instead.

A release step still requires someone to run a release. What makes it satisfy the
clause is that it is **bound to an action already being taken**, and that the
document requiring it cannot quietly drop the requirement, because the D4 checker
holds that document to the Gate Matrix. That is weaker than a daemon and stronger
than a reminder, and it is the best this project's own measured history supports.
Recorded plainly rather than claimed as automation.

## What this RFC leaves behind

`release-process.md:104-106` now reads: confirm the **`CI`** run — *"not some other
run on the same commit; check the job count against `[gates]`, not just that a run
reports success."*

That sentence exists because the architect nearly tagged 0.79.0 on a green
`Security audit` run — one job — that reported `success` identically to the 23-job
`CI` run on the same commit. The near-miss became procedure in the document this
RFC governs, which is the outcome RFC 131 was written to produce: **a claim about
the shipped system is checkable, and the check is written down where the person
making the claim will meet it.**

## Closure

`accepted/` falls to four — 094, 095, 096 and 117, all blocked on substantial
unbuilt work. **Every RFC that could be closed, is.**
