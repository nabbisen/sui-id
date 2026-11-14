# Verification Artifacts

This directory holds formal and semi-formal verification artifacts that are
useful for design review but are not part of the release package.

- `refresh_rotation.tla` models refresh-token rotation safety properties.
- `pilot-reports.md` records the RFC 086 pilot results and recommendations.
- `verus-scoping-note-2026-11-14.md` proposes one first Verus target, TOTP
  step selection, with a proof that was run, and what adopting it costs. It
  became [RFC 146](../rfcs/accepted/146-totp-step-selection-is-proved.md),
  accepted 2026-11-14.

Keep these files in sync with the corresponding Rust protocol code when the
protocol changes.

## Formal verification in sui-id: its position and how it is used

Raised by `@nabbisen`, 2026-11-14, on accepting RFC 146: *"I wonder if it is
better for us to update documentation to define what is verus's position
and how verus is used in the project from view of long term development and
maintenance."*

Its first part was settled by accepted RFCs and earlier rulings, cited
there. Its second was the architect's proposal for the rest, **approved
2026-11-14** (`@nabbisen`: *"Approved."*). **Both parts are now the project's
position.**

### Decided

- **Start from the smallest target, and widen one target at a time.**
  `@nabbisen`, 2026-11-10: *"Too wide range will cost insanely. It should
  start from minimal and be optimized every time."*
- **The first target is TOTP step selection**, and production calls the
  same pure function the proof is about. A CI lane fails when the two
  copies differ (RFC 146 §D1, §D3).
- **Verus runs on a maintainer's machine, not in CI**, until that target has
  paid for itself. Any change to a proved function's body carries a fresh
  Verus run in its review package (RFC 146 §D4).
- **Every target goes through an RFC**, and the feature freeze applies to
  building it (ROADMAP, *"After K: a scoping note on formal verification"*).
  RFC 146's build waits for M7, the soak.

### The rest: proposed by the architect, approved 2026-11-14

1. **Its position.** Formal verification is an assurance layer for a few
   small kernels, where one security property must hold for every input.
   **It complements tests and never replaces them.** Tests still own
   behaviour, integration and everything a proof assumes. A proof is worth
   having only where sampling cannot reach: an input space too large to
   enumerate, and a failure at a boundary or an extreme value.
2. **What qualifies as a target.** All of:
   - pure: no I/O, no `async`, no database, no clock read inside it;
   - small: tens of lines;
   - one security property, statable in a sentence;
   - an input space tests can't cover;
   - rarely changed.

   Integer logic is the cheapest case. Strings cost more and come later.
   **Cryptographic primitives are assumed, not proved**: the project relies
   on reviewed crates for them.
3. **What does not qualify:**
   - whole modules, HTTP handlers, or store code;
   - anything whose property depends on SQLite, the filesystem or
     concurrency (crash safety, transactions);
   - anything that only restates a test that already enumerates its domain.

   Protocol interleavings belong to a model (`refresh_rotation.tla`) or to
   state-machine tests, not to Verus.
4. **What each target must carry, for as long as it exists:**
   - its proof under `verification/verus/`;
   - the Verus version it was last proved with, recorded here;
   - a drift lane;
   - a fresh run in the review package of any change to its body;
   - a record of each planted bug the proof refuses, so it is known to be
     able to fail.

   **A proof that nobody runs is removed, not kept**, through an RFC, with
   its lane. That is the lesson of RFC 086's Kani harnesses
   (`crates/sui-id-core/src/identity/authz.rs:197`), which nothing in the
   repository runs.
5. **Who does what:**
   - the architect proposes a target, with a prototype that has actually
     been run;
   - it is authorized as any RFC is, by acceptance;
   - the implementation role keeps the proof in step with the code;
   - the reviewer checks the run output, not the claim.
6. **When to widen.** A second target is considered only after the first
   has gone through **at least one real change**, and the cost of that
   change has been measured and recorded here: time spent on the proof,
   and whether the drift lane or the proof caught anything. Then one target
   at a time. **When to run Verus in CI** is judged the same way, after
   that measurement.
7. **The toolchain stays separate.** Verus pins its own Rust toolchain. It
   never changes the project's MSRV or its `stable` lanes, and a Verus
   upgrade is re-proving every target, recorded here.
8. **The exit.** If Verus stops being maintained, or a target costs more
   than it protects, the proof and its lane are retired together, through
   an RFC. The code they covered keeps its tests.
9. **The Kani harnesses** at `authz.rs:197`: wire them into a scheduled job,
   or remove them. This is a separate, small decision. Until it is made,
   they prove nothing, because nothing runs them.

### Verus versions

| Target | Proof | Last proved with | On |
|---|---|---|---|
| TOTP step selection (RFC 146) | the scoping note's §2 until RFC 146 is built, then `verification/verus/select_step.rs` | `0.2026.11.02.8bf68ee` | 2026-11-14 |
