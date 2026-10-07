# Developer Handoff — RFC 138: `scripts/hunk-hashes.py`

## Role and protocol

**Addressee: the mid-capability model (dev team).** Authority on your role is
`.git-exclude/roles/mid-capability-model-operating-instructions.md`.

**On pushing:** `project-instructions-general-common.md:44` authorizes **both**
roles to commit and push. This dispatch asks only that you hand the tree over
uncommitted.

**Clone under `.git-exclude/tmp/clones/`, on `/home`.**

## RFC

**`rfcs/done/138-per-hunk-hashes-are-not-reproducible.md` — Accepted
2026-10-07.** Read it first; it carries the measurement and the reasoning. This
handoff builds D1 and D2 and respects D3.

## D1 — the tool

`scripts/hunk-hashes.py`:

```
python3.14 scripts/hunk-hashes.py --baseline <rev> [paths…]
```

**Output:** per changed path, the hunk count and one SHA-256 per hunk, in a
form that pastes straight into a package's Hashes section, plus a **labelled**
full-content hash for each added file. Match the shape packages already use, so
the tool's output *is* the section rather than something to transcribe.

**The rule, which belongs in the module docstring** — RFC 138 D1 states it and
the docstring is where it becomes executable:

> A hunk is its `@@` header line followed by its body lines, **each line
> including its terminating newline, the last one included**. Hunks are cut at
> `@@` headers; no newline belongs to a boundary. The diff is `-U3` against the
> stated baseline. The `diff --git`/`index`/`---`/`+++` preamble belongs to no
> hunk.

**Say in the docstring why that rule and not the other one:** a hunk's bytes
must not depend on whether another hunk follows it. The retired method broke
exactly that.

## D2 — the self-tests

`scripts/tests/test_hunk_hashes.py`. **The decisive case, which the retired
method fails:**

> **Hunk 1's hash is unchanged when a third hunk is appended after it.**

Write that one first; it is the regression this RFC exists to prevent. Then:

- a one-hunk file;
- an added file — full-content, labelled;
- a **deleted** file, and a pure **rename**: state in the docstring what the
  tool does with each rather than leaving it to be found later;
- a file whose final line has **no trailing newline** (`\ No newline at end of
  file`) — say what that does to the hash;
- **binary and CRLF content if cheap**; if not cheap, say so in the package
  rather than leaving the reader to assume it was covered.

They join `python3.14 -m pytest scripts/tests/`, which is run before every
commit.

## D3 — do not re-hash history

**Do not touch any declared hash in an existing package.** They were correct
under the method that produced them, and rewriting them would destroy the only
record of what was actually submitted. RFC 138 D3 is explicit.

## The dispatch template

Dispatches currently say *"Per-hunk SHA-256 over unified-diff text (`@@` header
+ body)"*. **That sentence is the defect.** Where a template or README carries
it, replace it with an instruction to run the tool and paste its output.

**Find them rather than guess:** `grep -rn "per-hunk SHA-256" rfcs/` — and
**change only templates and READMEs, never a dispatch already issued or a
package already submitted.** A superseded document's text is part of the
record.

## One more line, from RFC 138's second finding

`scripts/check-rfc-integrity.py:479` tells a reader that work no RFC governs
*"belongs in `roadmap/`, authorised by ROADMAP.md"*. **`roadmap/` was retired
on 2026-09-22** (`rfcs/README.md:107-109`). The message should say that such
work **needs an RFC**. It is a string change; the condition itself is right and
caught this RFC's own first draft.

**If changing it breaks a self-test that pins the message, update the test** —
that is the test doing its job, not an obstacle.

## Gates

**Full `python3.14 -m pytest scripts/tests/`**, A3.4, the three A3.2 `.sh`
fixture suites, `generate-ci-workflow.py --root . --check`, plus `G11`, `G14`,
`G17`, `G18`. **No Rust is touched, so the cargo lanes are not conditions** —
but if you touch anything under `crates/`, stop and say why.

Clean tree, clone on `/home`.

## Package

The usual — **and compute the hashes twice**: with the new tool, and with the
method you used before it existed. **Show both.** If they differ on a non-final
hunk, that difference is the defect being fixed and is the most useful line in
the package.
