# Cycle H and `0.86.0` — scope proposal

> ## Whose turn it is: **yours, for the scope decision.**
> **`@nabbisen`, 2026-10-28: _"Scope 0.86 and the proceed on the RFC and
> the other(s)."_** This is the scope; the RFC it names is written
> alongside it.

**Measured 2026-10-28** on `4e68f0e`, after `0.85.0` shipped. Filed here
for continuity with cycles E, F and G, though **`0.86.0` is not only RFC
094 work** — it spans three RFCs and one piece of release machinery.

## A — what is actually left, measured per item

| | Size | Blocked on |
|---|---|---|
| **M2b wave 3** — federation configuration | **small**: two live writes | nothing, since the 2026-10-25 ruling |
| **M2b item 6** — the authority switch | **large**: a new `xtask` crate | wave 3 (it exists to catch a *new* bare write) |
| **RFC 140 stage 1** — three app-identity link fields | small | nothing; accepted 2026-10-27 |
| **Trusted Publishing** | medium | an RFC, and six crates.io settings |
| **The local matrix is not written down anywhere** | small | nothing |

**Wave 3 is smaller than its row in the inventory suggests.** Measured
today: the only production writes are `runtime/startup.rs:323`
(`federation_provider::create`) and `:385` (`update_allowed_origins`),
both emitting no audit record at all. `set_enabled` and `delete` have no
production caller, and C23 has no write to convert — it arrives with
RFC 096-B2 and is sealed on arrival. So wave 3 is roughly one wave-4
stage, not four.

**The authority switch is the largest single thing left in RFC 094.**
`cargo xtask audit-structure` does not exist — there is **no `xtask`
crate in the workspace** — and RFC 094 `:433` and `:749` require it with
negative fixtures and CI wiring, plus making the structural gate
blocking and downgrading the string gate. `contracts/contract-paths.toml:78`
records it as *"planned … and not yet built"*. That is a cycle of its
own.

## B — the proposal

**`0.86.0` = the deferred work, plus the machinery that keeps this
programme honest. Five items, none large.**

1. **M2b wave 3 — federation configuration. DISPATCHED 2026-10-28** —
   [`m2b-wave3-federation-configuration-2026-10-28.md`](m2b-wave3-federation-configuration-2026-10-28.md).
   Seal the two live startup writes onto the Class-A seam, emitting the events the inventory
   already names, with the target semantics left to RFC 096-B2 per the
   2026-10-25 ruling. Dispose of `set_enabled`/`delete`/C23 the way
   stage 4 disposed of C10/C22/K02. **After this, the only thing
   standing between M2b and closure is the authority switch.**
2. **RFC 140 stage 1** — `homepage_uri`, `policy_uri`, `tos_uri`
   settable by an administrator and rendered on the consent screen for
   administrator-registered clients. Accepted 2026-10-27; §5's logo
   decision stays open and is not in this cut.
3. **Trusted Publishing** — [RFC 141](../../accepted/141-publishing-is-a-workflow-not-a-credential.md),
   written alongside this scope. Build the workflow in `0.86.0`; **first
   use it at `0.87.0`**, not on its own release, because the first run
   of new release machinery should not be on a release already waiting.
4. **The local matrix becomes a script. DISPATCHED 2026-10-28** —
   [`../093-build-toolchain-release-gates/local-matrix-script-2026-10-28.md`](../093-build-toolchain-release-gates/local-matrix-script-2026-10-28.md),
   filed under RFC 093 because that RFC owns the Gate Matrix. There is
   **no tracked definition of it anywhere** — I grepped `docs/`, the rules and the
   role files. It lives in whatever each dispatch happens to enumerate,
   which is why four consecutive stages ran a matrix that was missing
   fourteen `unittest` steps, and why the round that found them
   enumerated thirteen. A script that derives the set from
   `contracts/gate-inputs.toml` and `.github/workflows/ci.yml` and runs
   it makes the dispatch say *run this* instead of listing commands I
   have to keep correct by hand.
5. **Three tidies**, each a few lines, folded into whichever round
   touches the file: the nanosecond timestamps in `cli.rs`'s two
   renderers (`cli.rs:301` already uses the shorter form); a test for
   `render_registration_token_list`'s empty branch, which is what an
   operator sees on their first run; and the `sealed_by`/`target-absent`
   rows re-read once more when wave 3 lands.

**`0.87.0` = the authority switch, M2b closes, and it is the first
release published through RFC 141's workflow.** Named here so the
sequencing is a decision on the record rather than a drift.
[RFC 142](../../proposed/142-upgrading-is-the-least-tested-thing-we-do.md)
— upgrade safety — is **Proposed** and not scheduled; it is a candidate
for `0.87.0` or later.

## C — why this order

**Wave 3 before the authority switch** is RFC 094's own argument
(`:1101-1105`): the gate exists to catch a newly introduced bare write,
and that value exists only once the boundary it protects is complete.
Building it while a wave is still unconverted means building it against
a boundary still moving.

**Trusted Publishing before it is needed, not during.** `0.85.0`'s
publish was blocked at the last step by a permission guard doing its
job, and the fix is not to work around the guard but to move the publish
somewhere a guard is the right shape — a workflow with an approval
gate. Building that under time pressure at a cut is how it gets built
wrong.

**The matrix script before the next programme, not after.** Every one of
the five rounds in `0.85.0` reported "30/30" and every one of them was
measuring something slightly different from CI. That is a defect in my
dispatches, repeated five times, and the fix is to stop writing the list
by hand.

## D — what I need from you

1. **`0.86.0` = those five?** Or a different cut — in particular, if you
   would rather close M2b sooner, say so and I will propose wave 3 and
   the authority switch together as one larger cycle, with the
   consequence that it is the longest cycle this programme has run.
2. **RFC 141 — accepted 2026-10-28.** §6's six crates.io configurations
   are yours and are not implementation; the workflow and its refusal
   fixtures can be built before they exist. §7 is settled: **`0.87.0`** is the
   first release published through the workflow, so `0.86.0` is
   published by the existing path.
3. **Nothing else.** The stage plans, the dispatches and the release
   cycle are mine once the scope is authorized.
