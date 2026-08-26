# Reconcile `rfcs/README.md`'s template with what G11 enforces

**Tracks.** Defect exposed by RFC 093 M1b's own gate.
**Owner.** Implementation.
**Baseline.** `8d446b0`.
**Size.** Small, but **larger than "one field"** — see below. I have described this
as a one-field fix three times; measuring it shows ten labels missing, one of them
gate-enforced.

## The live defect

`rfcs/README.md` §Template is the **normative** template — RFC 093's integrity
contract says the parser recognises "the bold, period-terminated metadata labels
from the README template."

G11 requires seven fields on any standard RFC numbered ≥ 093. The template
supplies six:

| G11 requires | In template |
|---|---|
| Security review, Design/Implementation/Closure prerequisites, Tracks, Touches | yes |
| **Accountable owner and approver** | **no** |

**So an author who follows the documented template writes an RFC that fails the
gate.** The gate and the documentation disagree, and the documentation is the part
a contributor reads. That is the whole thesis of RFC 098, live in one file.

## What else has drifted

Measured across the RFCs in `proposed/` and `accepted/` — labels used in practice
but absent from the template:

| Label | Used in |
|---|---|
| `Handoff` | 8 RFCs |
| `Accountable owner and approver` | 8 RFCs |
| `RFC author / architect` | 8 RFCs |
| `Independent security and closure reviewer` | 7 RFCs |
| `Lifecycle history` | 3 |
| `Amended on` | 3 |
| `Priority`, `Validation matrix`, `Command inventory`, `Independent security and soak/closure reviewer` | 1–2 each |

## What to do

**1. Add `**Accountable owner and approver.**` to the required block.** This is the
only change the gate forces, and the only one that is currently a trap.

**2. Add the three other established labels** — `Handoff`, `RFC author /
architect`, `Independent security and closure reviewer`. Seven or eight uses each
is convention, not accident, and a template that omits them teaches the wrong
shape.

**3. Add `Amended on` and `Lifecycle history` in the commentary**, not the required
block. Both are real governance fields — `Amended on` is explicitly repeatable, and
RFC 093 carries three of them — but neither applies to a new RFC. Show them as
"add when amending" / "add when returning to `proposed/`", alongside the existing
`<!-- Add when moving to accepted/: -->` convention the template already uses.

**4. Leave the one-offs out.** `Priority`, `Validation matrix`, `Command inventory`
are specific to individual RFCs. A template is the common shape, not a union of
everything anyone has written.

## Verify

```bash
python3.14 scripts/check-rfc-integrity.py --root . --policy ci/rfc-policy.toml
python3.14 scripts/check-markdown-links.py --root . README.md ROADMAP.md docs
```

Both must stay green — `rfcs/README.md` is the index G11 reads, so a malformed
edit there fails the gate rather than merely looking wrong.

**The real check is different, and worth doing:** take the corrected template,
write a throwaway RFC from it numbered ≥ 093, and confirm G11 *accepts* it. Then
delete the throwaway. The defect is that the template produces failing RFCs, so
the test is whether it now produces a passing one — not whether the file still
parses.

## Scope

`rfcs/README.md` only. Do not touch existing RFCs to match: they already carry the
fields, and this is a documentation repair, not a migration.
