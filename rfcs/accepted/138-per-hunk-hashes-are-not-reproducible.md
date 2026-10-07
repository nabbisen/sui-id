# RFC 138 — Per-hunk hashes are not reproducible

**Status.** Accepted
**Accepted on.** 2026-10-07
**Approved by.** `@nabbisen`, 2026-10-07: "RFC 138 is accepted."
**Security review.** Not required — no application behaviour changes and no crate's public surface moves. This RFC defines how a review package's evidence is computed and adds one script with self-tests. Reason subject to acceptance.

**Design prerequisites.** None.
**Implementation prerequisites.** This RFC Accepted — **satisfied 2026-10-07**. Dispatched the same day.
**Closure prerequisites.** One script computes every hash a package declares; its definition of a hunk is in its own docstring; a self-test fails if a hunk's hash changes when a later hunk is appended; and every dispatch cites the script instead of restating the method in prose.
**Tracks.** Verification integrity.
**Touches.** `scripts/`, `scripts/tests/`, `contracts/README.md` if a row is required, and the handoff template's Package section.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.

*Retitled 2026-10-07, minutes after acceptance, from "The verification method
has no owner". Cosmetic and non-material — no scope, invariant or prerequisite
changed, and the number the acceptance names is unchanged. The reason is this
RFC's own subject: the old title contained the word "owner", so every citation
of it matched G16's attribution pattern and earned a baseline line of pure
filename noise. Three such lines appeared within minutes, and the count would
have grown with every document citing it. **A ledger whose value is being
readable should not be fed by a naming choice.** Recorded as a guideline: an
RFC title should avoid the owner token.*

## Summary

**Every review package in this project is verified by per-hunk SHA-256 hashes,
and nothing defines how to compute one.**

The phrase *"per-hunk SHA-256 over unified-diff text (`@@` header + body)"*
appears **only inside individual handoffs**. It is in no RFC, no file under
`.git-exclude/rules/`, and no contract. Each implementer reads it and picks an
interpretation; each reviewer does the same.

**Measured 2026-10-07, reviewing RFC 096-A stage 3b.** Two honest readings of
that sentence disagree on one byte — the newline that ends a hunk:

| Method | `jwks.rs` hunk 1 | hunk 2 |
|---|---|---|
| split the diff text on `"\n@@"` | `fb7d54fa…` | `21e12820…` |
| `@@` header + body lines, every line newline-terminated | `62728fdf…` | `21e12820…` |

**The last hunk agrees; the first does not.** Splitting on `"\n@@"` consumes
the newline that ended the preceding hunk, so:

> **An independent recomputation can only ever confirm the final hunk of a
> multi-hunk file.** Every earlier hunk mismatches **even when the content is
> byte-identical** — confirmed with `cmp` against a copy before concluding
> anything about the content.

## Why this is worth an RFC rather than a quiet fix

**The mechanism it breaks is the one the project relies on most.** Per-hunk
hashing is how a reviewer establishes that the package they read is the tree
they are looking at. In its current shape it manufactures **false mismatches**,
and a false mismatch is worse than no check: it spends a review cycle on a
defect that does not exist, and it teaches the reviewer to distrust the tool.

**That already happened twice in one session on 2026-10-06**, and both times
the architect attributed it to its own scripts rather than to the convention —
once to `git status --porcelain` collapsing untracked directories, once to
reading a hash without its `(new file, full content)` label. **Neither
diagnosis was wrong, and neither was complete**, because the convention
underneath was never examined.

**And the governance gap is the real defect.** A rule nobody owns cannot be
corrected, because there is nowhere to correct it. Two implementations
disagreeing invisibly for weeks is the predictable outcome of a contract that
exists only as a sentence copied between dispatches.

## D1 — The tool is the definition

**Ship one script that both roles run.** Prose is what failed; a second, more
careful sentence would fail the same way.

`scripts/hunk-hashes.py --baseline <rev> [paths…]` prints, per changed path,
the hunk count and one SHA-256 per hunk in a form that pastes into a package,
plus a labelled full-content hash for each added file.

**The rule it implements, stated in its docstring:**

> A hunk is its `@@` header line followed by its body lines, **each line
> including its terminating newline, the last one included**. Hunks are cut at
> `@@` headers; no newline belongs to a boundary. The diff is `-U3` against the
> stated baseline. The `diff --git`/`index`/`---`/`+++` preamble belongs to no
> hunk.

**Chosen because a hunk's bytes do not then depend on whether another hunk
follows it** — the property whose absence caused this.

## D2 — A self-test for the property that broke

A tool with no committed negative test agrees with itself and nothing else.
The decisive case:

> **Hunk 1's hash is unchanged when a third hunk is appended after it.**

The retired method fails that test. So must the suite cover a one-hunk file, an
added file, a deletion, a rename, and a file whose last line has no trailing
newline — each with the tool's behaviour stated rather than discovered.

## D3 — Existing packages are not re-hashed

**Declared hashes in packages already submitted stay as they are.** They were
correct under the method that produced them. Rewriting them to match a new tool
would destroy the only record of what was actually submitted, which is the
thing the hashes exist to preserve.

## Non-goals

- **No CI gate.** This is a tool the two roles run when producing and checking
  a package, not a lane. A gate would have to re-derive a baseline it cannot
  know.
- **No change to what a package contains**, only to how one number in it is
  computed.

## A second finding, recorded here because it has the same shape

`scripts/check-rfc-integrity.py:479` refuses a handoff directory that does not
name an RFC and advises that such work *"belongs in `roadmap/`, authorised by
ROADMAP.md"*. **`rfcs/README.md:107-109` records that `roadmap/` was retired on 2026-09-22**
and that implementation handoffs live under `rfcs/handoffs/` from then on; the
ruling behind it is recorded there, not restated here.

**The gate's own error message points at a directory the project abolished.**
It caught this RFC's first draft correctly and then sent it somewhere that does
not exist. Worth one line in the same change: the message should say that work
no RFC governs **needs an RFC**.
