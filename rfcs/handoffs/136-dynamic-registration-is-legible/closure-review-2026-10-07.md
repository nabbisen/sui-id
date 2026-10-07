# RFC 136 — closure review

**Date:** 2026-10-07. **Recommends closure. Does not close it.** The decision
lives in RFC 136's `Closure approved by.` field, not here.

*Framed this way deliberately. RFC 135's closure review opened by declaring
itself to be awaiting a sign-off — true when written, false once that arrived.
Its RFC's `Closure evidence.` field pointed at it, so the implementation role
followed the trail and reported a closed RFC as pending. A document cited as
evidence says what it **is**, never what is currently outstanding.*

**Independence.** Written by the architect, which wrote RFC 136, both its
dispatches and the closure criteria. **Not independent.** RFC 000 provides for
that: the accountable owner signs off. The implementation role verified its own
measurements and the Level B run separately; that is corroboration.

## The three prerequisites

RFC 136's own words: *"Both consent-policy defaults are recorded with their
reasoning in a place a reader of the code will find; the administrator-facing
client list distinguishes a self-registered client from an administrator-created
one; and a test fails if the distinction stops being shown."*

### 1 — both defaults recorded, with reasoning — **met**

| Default | Where | What it says |
|---|---|---|
| administrator-created | `crates/sui-id-core/src/identity/admin/clients.rs:91` | *"RFC 136 D1: `ConsentPolicy::default()` (→ `None`, no consent …)"* |
| dynamically registered | `crates/sui-id/src/http/handlers/dynamic_register.rs:264` | *"RFC 136 D1: `FirstTime`, **not** `ConsentPolicy::default()`"* |

Both sit beside the code that applies them, both cite D1, and the dynamic one
states explicitly what it is *not* — which is the part a reader needs, because
the two defaults differ on purpose.

### 2 — the list distinguishes self-registered from administrator-created — **met**

`crates/sui-id-web/src/pages/clients.rs:37` renders
`StatusKind::SelfRegistered` when `registered_via == "dynamic"`, and nothing
for `"admin"`. The same distinction is rendered again in the edit view at
`:280`.

### 3 — a test fails if the distinction stops being shown — **met, and proven by mutation**

**This was the prerequisite that was never dispatched.** Step 1 delivered 1 and
2; the test was simply missing, and the handoff index asserted that step 1
*"is the whole of RFC 136; there is no stage 2"*, which is what stopped anyone
looking. Found by a prerequisite sweep on 2026-10-07, dispatched as step 2,
landed in `7cd3298`.

**A passing test does not discharge this prerequisite** — it says a test must
*fail* when the marker goes. So each render site was mutated:

| Mutation | Result |
|---|---|
| badge removed from the list row (`:37`) | `render_clients_marks_a_dynamically_registered_client` **fails**; both edit tests pass |
| badge removed from the edit view (`:280`) | `render_client_edit_marks_a_dynamically_registered_client` **fails**; both list tests pass |

**Each site is caught by its own test and only its own.** That matters because
the expression `(registered_via == "dynamic")` is written out at both sites —
duplication that two tests would have left half-covered.

`sui-id-web` went from **3 tests to 7**. The assertions use
`lang.strings().status_self_registered` rather than a literal, so a translation
change cannot raise a false alarm.

## Level B

**`7cd3298`, run `37593404727` — 27 jobs, 0 skipped, all green.** Verified by
me to cover all **24** entries in `[gates]`, not inferred from the count, and
independently re-checked by the implementation role.

**Unlike RFC 135, this one evidenced itself.** RFC 135's closing fix touched
only `contracts/owner-attributions.toml`, outside the Rust lanes' `paths`, so
its own run was path-filtered to Level A and it had to wait for an unrelated
commit. Step 2 touches `crates/**`, so the commit that completes the RFC is
also the commit that proves it.

## What this RFC delivered, and the honest part

**Legibility, not behaviour.** Two consent-policy defaults that differ on
purpose now say why, where a reader of the code will find it, and an
administrator can see which clients registered themselves.

**The marker is not cosmetic.** Self-registered versus administrator-created is
what an administrator uses to spot a client nobody authorised. A marker that
silently stopped rendering would remove that signal while leaving the page
looking correct — which is precisely why prerequisite 3 exists and why it was
worth proving by mutation rather than by a green test.

**Recorded against the architect:** step 1's review said *"accepted, no
required changes"*, which was right for what step 1 was dispatched to do, and
the handoff index then asserted completeness the prerequisites contradicted.
RFC 136 sat one unbuilt test short of closable for two days, and nothing would
have surfaced it except reading the prerequisites against the code.

**Also recorded:** during step 2's review the architect destroyed the returned
package's uncommitted `mod tests;` declaration with `git checkout --`, nearly
misread the resulting test-count drop as a weak test, and recovered it —
verified byte-identical by recomputing the package's own declared hunk hash.

## Recommendation

**Close to `done/`, Status Implemented**, on the owner's sign-off.
