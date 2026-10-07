# RFC 138 — handoff

**RFC.** [`../../done/138-per-hunk-hashes-are-not-reproducible.md`](../../done/138-per-hunk-hashes-are-not-reproducible.md)

**Status: Implemented, closed 2026-10-08** — the RFC header carries the
approval and the Level B evidence. One stage, landed `606704a`.

## Open work

**None.**

## Landed

- [`hunk-hashes-tool-2026-10-07.md`](hunk-hashes-tool-2026-10-07.md) —
  **landed `606704a`.** `scripts/hunk-hashes.py`, 14 self-tests, and four
  handoff READMEs citing the tool instead of restating the method.
- [`closure-review-2026-10-08.md`](closure-review-2026-10-08.md) — the closure
  record.

## Why this RFC exists

Per-hunk SHA-256 is how a reviewer establishes that the package they read is
the tree in front of them, and **nothing defined how to compute one** — the
phrase appeared only inside individual handoffs. Two honest readings disagreed
on the newline that ends a hunk, so an independent recomputation could only
ever confirm the **final** hunk of a multi-hunk file. Found 2026-10-07 while
reviewing RFC 096-A stage 3b, where the content was byte-identical and the
hashes still disagreed.
