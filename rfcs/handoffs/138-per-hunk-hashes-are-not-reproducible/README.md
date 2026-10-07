# RFC 138 — handoff

**RFC.** [`../../accepted/138-per-hunk-hashes-are-not-reproducible.md`](../../accepted/138-per-hunk-hashes-are-not-reproducible.md)

**Status: Accepted 2026-10-07**, dispatched the same day. One stage.

## Open work

- [`hunk-hashes-tool-2026-10-07.md`](hunk-hashes-tool-2026-10-07.md) —
  **DISPATCHED, this is the open work.** `scripts/hunk-hashes.py` plus its
  self-tests, and the dispatch template citing the tool instead of restating
  the method in prose.

## Why this RFC exists

Per-hunk SHA-256 is how a reviewer establishes that the package they read is
the tree in front of them, and **nothing defined how to compute one** — the
phrase appeared only inside individual handoffs. Two honest readings disagreed
on the newline that ends a hunk, so an independent recomputation could only
ever confirm the **final** hunk of a multi-hunk file. Found 2026-10-07 while
reviewing RFC 096-A stage 3b, where the content was byte-identical and the
hashes still disagreed.
