# Work packages authorized by the roadmap

Execution packages for project work that changes the codebase but decides
nothing about the product's design or security posture: toolchain repairs,
test organization, fuzz infrastructure, gate-template defects, preparatory
refactors.

## What authorizes the work here

[`ROADMAP.md`](../ROADMAP.md) — owner ruling, 2026-09-10. A package in this
directory is authorized by an entry in the roadmap's *Non-RFC work packages*
section, or in *Execution order* for items that are prerequisites of the
remediation programme. **A package with no roadmap entry is not authorized
work**, and a roadmap entry with no package is a broken link.

## Why not `rfcs/handoffs/`

RFC 000 defines a handoff as "an optional implementation companion **to an
RFC**" and requires that "every `rfcs/handoffs/NNN-slug/` directory ...
corresponds to an existing RFC number." A handoff also inherits its RFC's
lifecycle status, which is wrong for work that no RFC governs: filing live
work under a closed RFC would mark it historical, and filing a prerequisite
under the RFC that depends on it inverts the dependency.

Six packages sat under `rfcs/handoffs/` with no RFC until 2026-09-10. That is
how work with no decision record became invisible — nothing indexed it and no
gate could see it. G11's invariant 13 now enforces the correspondence rule, so
it cannot recur.

## Why not `docs/`

These are instructions for executing work, not documentation of the product.
`docs/` answers "how do I run, integrate with, or contribute to sui-id"; a
work package answers "how do we carry out this specific change, and how will
we know it worked." A package is finished and inert once its work lands; a
document is expected to stay true.

## Layout

```
roadmap/
  <slug>/
    README.md        ← the package: scope, entry gate, constraints, evidence
    <extra>.md       ← optional, when one file is not enough
```

No status subdirectories. A package's state is recorded in the roadmap entry
and in the package's own header — not by which folder it sits in. Duplicating
state across a folder and a document is what made `rfcs/reviews/` and RFC 018
harmful.

## Current packages

| Package | State |
|---|---|
| [prep-federation-module-split](prep-federation-module-split/README.md) | Withdrawn 2026-08-12 — ⛔ not ready to execute; roadmap *Execution order* item 5 |
| [rfc-template-reconciliation](rfc-template-reconciliation/README.md) | Open, unstarted |
| [test-file-organization](test-file-organization/README.md) | Open — §A and §B done, §C queued |
| [fuzz-widen-matrix](fuzz-widen-matrix/README.md) | Complete, `d5e5402` |
| [fuzz-corpus-persistence](fuzz-corpus-persistence/README.md) | Complete, `8d446b0` |
| [stable-clippy-drift-1.98](stable-clippy-drift-1.98/README.md) | Complete, `b343a06` |
