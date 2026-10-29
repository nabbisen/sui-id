# Cycle I and `0.87.0` — scope proposal, and a correction

> ## Whose turn it is: **yours, for the scope decision.**
> **`@nabbisen`, 2026-10-29: _"Scope 0.87."_**

**Measured 2026-10-29** on `35e1b07`, after `0.86.0` shipped.

## A — the correction, and it is the centre of this document

**I have said, repeatedly and in the ROADMAP, that "only the authority
switch stands between M2b and closure." That is wrong.**

Measured today against `contracts/write-commands.toml`: **nineteen
Class-A rows are `sealed = false` with no `sealed_by`.**

```
U11 U13 U16 U17 U18 U19 U20 U21 U26
C12 C13 C14 C17 C18 C19 C20 C23
S10 K02
```

**How I got it wrong.** RFC 094 `:1093-1098` lists M2b as item 5, *"the
remaining conversion waves: server and security settings; pending
sensitive settings; federation configuration; client administration
metadata"*, and item 6, the authority switch. I scoped M2b as **those
four waves**, finished them, and concluded only item 6 remained.

**But M2b's closure field (`:54`) does not say "the four waves." It
says "every remaining Class-A row converted."** The waves are a
description of the bulk of the work, not an enumeration of the
criterion. Nineteen rows sit outside all four and inside the criterion.

This is the exact failure my own standing note warns about — *a phase
ends when the RFC's prerequisite is met, not when my stage list runs
out* — and I made it anyway, across four releases.

**What this does not mean.** M2a did not close improperly. Its criteria
(`:54`, consolidated 2026-10-03) are about *"every **converted**
Class-A path"* — the paths it chose to convert — not exhaustiveness.
M2a converted its priority waves correctly. The remainder was always
M2b's by the closure text; I simply never read the closure text against
the inventory.

## B — what the nineteen actually are, and what I have not measured

**Five have a live production caller**, found today:

| Row | Live write |
|---|---|
| U19 | `sessions::revoke_all_for_user` |
| U21 | `user_consent::upsert` — `handlers/oidc.rs:917`, the consent grant |
| C14 | `client_registration_token::create` — the issue CLI |
| C19 | `federation_link::upsert` |
| S10 | `state::mark_initialized` — first setup |

**Three are already dispositioned** and need no conversion: C17 and
C18 (federation enable/disable/delete — no production caller, awaiting
RFC 096-B2), C23 (no function exists), K02 (removed as redundant).

**The remaining eleven appear to have no production caller** — U11,
U13, U16, U17, U18, U20, U26, C12, C13, C20 and one more — on greps
that returned nothing for both the qualified path and the bare
identifier.

**I do not trust that last group and neither should you.** This
programme has found a caller a grep missed **three times**: `revoke`
reached through `use super::*`, `update_app_identity` whose caller
count I asserted before reading the store, and the `-eq 0` polarity
heuristic. **An inventory census is the first work of this cycle, not
an assumption behind it.**

## C — the proposal

**`0.87.0` = finish M2b. Four items.**

1. **The census.** For each of the nineteen, measured per row with a
   `file:line`: live caller or none; if live, whether it appends
   best-effort or not at all. **Its output is the stage plan for items
   2 and 3**, and it is explicitly allowed to overturn §B.
2. **The live conversions.** On today's measurement that is five rows;
   the census decides the real number.
3. **The dispositions.** Callerless rows get the treatment C10, C22,
   K02 and C17/C18 got — wire up, remove, or reserve, decided per row
   and recorded in the row's own rationale.
4. **The authority switch**, last. RFC 094 `:1101-1105` is explicit:
   the AST gate exists to catch a *newly introduced* bare write, and
   building it before the boundary is complete is premature. Items 2
   and 3 are that boundary. It needs a new `xtask` crate —
   `cargo xtask audit-structure` does not exist and there is no `xtask`
   in the workspace.

**Then M2b closes**: every remaining Class-A row converted, no
production Class-A best-effort append anywhere, the AST gate landed
with a negative fixture, and your acceptance on my adversarial
assessment.

**`0.87.0` is also RFC 141's first use.** Two things must land before
the cut, both already recorded: `release-process.md` documents the
workflow, and the six crates.io Trusted Publishing configurations are
yours. Trusted-Publishing-only mode and revoking the workstation token
come only **after** the first successful workflow publish.

## D — the honest alternative

**This is a large cycle, and I have just demonstrated that my size
estimates for it have been wrong.** If you would rather it be two:

- **`0.87.0`** = the census, the conversions and the dispositions —
  M2b's conversion criterion met, published through the new workflow.
- **`0.88.0`** = the authority switch alone, and M2b closes there.

**I recommend the split.** The authority switch is the largest single
unbuilt thing in RFC 094, it needs a new workspace member, and pairing
it with a body of work whose size I cannot yet state is how a cycle
becomes open-ended. The split also means the first release published
through the new workflow is a smaller one, which is the right release
to discover a problem on.

## E — what I need from you

1. **One cycle or two?** Recommended: two, as §D.
2. **Nothing else.** The census, the stage plans and the dispatches are
   mine once the scope is authorized — and the census comes first
   precisely because the plans should not be written before it.

**Not in this scope, and still open**:
[RFC 142](../../proposed/142-upgrading-is-the-least-tested-thing-we-do.md)'s
two questions, and RFC 140 §5's logo decision.
