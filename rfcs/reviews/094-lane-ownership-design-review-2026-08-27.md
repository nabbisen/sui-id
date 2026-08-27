# RFC 094 — multi-source lane ownership design — implementability review

**Date:** 2026-08-27 (Asia/Tokyo)
**Reviewer:** Mid-capability model, implementation role — did not author this
design; the author (requirements-architect role) is the reviewer of my
last two rounds, per the request's own framing.
**Request:** [`094-lane-ownership-design-review-request-2026-08-27.md`](./094-lane-ownership-design-review-request-2026-08-27.md)
**Baseline:** `3c16e81` — re-confirmed against this exact commit before
reading a single line, per the standing practice from the last two rounds.
**Subject:** RFC 094 §*Structural coverage gate*, subsections *Manifest
shape*, *What condition 7 becomes*, *Why ownership conflicts cannot
occur*, *Why an owning RFC needs no Gate Matrix table of RFC 093's shape*,
*Migration*.

## Outcome: Corrections Required

The single question asked — "could you build this from what is written,
without making a design decision of your own?" — the honest answer is
**not quite**: the design is buildable in its main shape, but the
behavior-preservation claim in §*Migration* is false as written, in two
independently verified, concrete ways. Both are fixable with an additional
check apiece, not a redesign. Claim B surfaces a third, smaller gap in the
extraction mechanism itself. Claim C's design is already correct — I could
not break it. Claim D needs an explicit decision, which I recommend but
don't make.

## Method

Every claim below that could be tested was tested, not argued. I built a
throwaway Python harness (removed after use, not committed — `git status`
confirms this repository's tree is untouched) that reimplements **both**
algorithms side by side: today's real `check-gate-inputs.sh` condition 7,
and the RFC's four-check design applied to a migration manifest mapping
every current lane to `"093"`. Both were run against the real, current
`ci/gate-inputs.toml` and RFC 093's real Gate Matrix v1 table, plus six
deliberate mutations.

One thing the harness caught in me before it caught anything in the RFC:
my first extraction pass disagreed with the real script on the live data
(`old_ok=False` on unmodified input, which is impossible — the real
checker reports "all conditions satisfied"). The cause was my own bug, not
the RFC's: I hadn't applied the one normalisation the script performs
(`` ` and ` `` → `` ` && ` `` for G05/G06's two-command cells, documented at
`check-gate-inputs.sh:240-243`). Fixed and re-verified against the real
script's actual "all conditions satisfied" output before trusting any
further result from the harness.

## Claim A — the migration is not behavior-preserving, in two specific ways

**Finding A1 — the disjointness invariant is dropped.**

Today's check explicitly rejects a lane present in *both* `[gates]` and
`[gate_matrix_exceptions]` — this is a named, distinct failure in
`check-gate-inputs.sh` (`"present in both [gates] and
[gate_matrix_exceptions]"`), independent of whether the `[gates]` command
happens to be correct.

The RFC's four checks do not reproduce this. Constructed the exact case:
`G12` added to `[gates]` with its **byte-correct** command (taken directly
from RFC 093's own table), while also left in `[gate_matrix_exceptions]`
unchanged.

| | Old (real) algorithm | New (four-check) algorithm |
|---|---|---|
| Result | **fails** — `present in both [gates] and [gate_matrix_exceptions]: {'G12'}` | **passes** — no check fires |

Walking the four checks against this exact input confirms why: check 1's
`[gate_owners]`-membership test is a union ("is a `[gates]` key **or** a
`[gate_matrix_exceptions]` key"), not a partition. G12 satisfies "or" by
being in either list, and nothing anywhere in the four checks tests that
the two lists are disjoint. Checks 2–4 have no opinion on set membership
overlap at all.

**This is exactly the failure mode the migration story depends on not
having**: an exception lane that's *also* accidentally left in `[gates]`
with a technically-correct command (e.g., mid-way through converting a
lane from exception to dispatched, if the exception removal is forgotten)
would silently pass under the new design where it fails loudly today.

**Fix:** add a fifth check — `[gates]` and `[gate_matrix_exceptions]` keys
are disjoint — or fold it into check 1 by changing "or" to "exactly one
of."

**Finding A2 — exception entries are no longer required to be grounded in
a real lane.**

Today's check also rejects a `[gate_matrix_exceptions]` entry for a lane
that doesn't appear in RFC 093's table at all (`"[gate_matrix_exceptions]
lane(s) not in RFC 093's table"`). Constructed the case: added a phantom
exception, `G404 = "..."`, for a lane ID that exists in no source RFC's
table.

| | Old (real) algorithm | New (four-check) algorithm |
|---|---|---|
| Result | **fails** — `[gate_matrix_exceptions] lane(s) not in RFC 093's table: {'G404'}` | **passes** — no check fires |

Check 3 only constrains the **forward** direction (every table lane must
be accounted for); nothing constrains the **reverse** (every exception
must correspond to a real table row). A fabricated exception — accidental
or a copy-paste error — currently cannot exist undetected. Under the new
design, it can.

**Fix:** add a sixth check — every `[gate_matrix_exceptions]` key appears
in some source RFC's table (the reverse of check 3). This is a small,
independently-addable check and is not entangled with Claim D's
open question about *owners* for exceptions — groundedness-in-a-table-row
and ownership are two different properties, and only the first is
currently missing regardless of how D is decided.

**With both fixes applied, I re-ran the harness against all eight cases
(the original six plus these two) and every case matched.** The migration
claim holds once these two checks are added; it does not hold as
currently specified.

## Claim B — heading parsing generalises with three unaddressed pitfalls

I did not build a full generalized extractor (out of scope, and the
question is about the *design's* completeness, not a specific
implementation's). Reasoning through the three scenarios named, against
today's actual awk mechanics (`check-gate-inputs.sh:250-254`):

**1. Regex metacharacters in a heading.** Today's extraction is a literal
awk pattern, `/^## Gate Matrix v1/`, hardcoded at authoring time — there is
no escaping concern because the pattern is written by a human who controls
both the regex and the text it matches. Once `[gate_lane_sources]` supplies
the heading as **data** read from TOML, whatever builds the match pattern
from that string must either regex-escape it before use in
awk/grep/Python `re`, or avoid pattern-matching entirely (e.g., compare the
heading's stripped text with plain string equality after stripping leading
`#` characters). The RFC's *Manifest shape* section doesn't say which, and
the difference matters: a heading like `Gate Matrix (v2)` fed unescaped
into a regex-based matcher would have its parentheses interpreted as a
capture group, silently matching something other than intended (or failing
to match the literal heading at all, depending on what follows).

**2. Heading level.** Today's start pattern is fixed at exactly `##`
(level 2). `[gate_lane_sources]`'s example value, `"Gate Matrix lanes owned
by RFC 094"`, carries no level information — it's bare text. RFC 094's own
current document structure uses `####`-level subsections nearby (`####
Manifest shape`, etc.), so an owning RFC choosing to nest its lane table
under a `###` or `####` heading is plausible, maybe likely. The design
doesn't say whether matching is level-agnostic (any `#{1,6}` prefix) or
implicitly still level-2-only. If it's meant to be level-agnostic — which
seems intended, since "a heading of its own choosing" reads as choosing
text, not being constrained to level 2 — that should be stated, since it's
a different regex than today's.

**3. Duplicate heading text.** This is the one I'd weight most heavily.
Traced today's awk state machine literally: `/^## Gate Matrix v1/ {
in_section = 1; next }` is an *unconditional* pattern match — it does not
check whether the section is already open, and it does not check whether
it has already closed once. If the identical heading text occurred a
**second time**, anywhere later in the same document, extraction would
silently resume, and the second occurrence's table rows would be merged
into the first's output — with no error, and with later rows winning on
any lane-ID collision in whatever data structure holds them (a dict-shaped
accumulator, for instance, silently overwrites; a list-shaped one silently
duplicates). Today this doesn't matter because the heading text is a fixed
literal a human controls in one document. Under the multi-source design,
each owning RFC picks its own heading text with no uniqueness constraint
stated — nothing stops an author from reusing a common heading phrase (or
from two structurally similar RFCs, written by the same drafting habit,
independently choosing the same heading text for unrelated content the
extractor was never meant to combine). **This should fail loudly (heading
matched more than once) rather than silently combine**, and the design as
written doesn't say it does either.

None of these three break the design's shape — they're implementation
requirements that need to be stated so whoever builds the extractor
doesn't have to infer them, the same shape of gap the sweep found in the
2026-08-26 version of this passage, smaller in scope this time.

## Claim C — already correct; I could not find a gap

Traced the "two folders or neither" scenario directly rather than
guessing. The design's own text closes it: "Resolution must yield
**exactly one** file — zero or several is a failure, not a guess."

- **Two folders (duplicate)**: `NNN-*.md` matched across `proposed`,
  `accepted`, `done`, `archive` would return two files, which the spec
  explicitly treats as a failure, not a silent pick.
- **Neither (missing)**: zero matches is explicitly a failure too.
- Since every real lifecycle move in this repository's actual practice
  lands as one clean commit (evidenced throughout this whole programme by
  `event_commit == checked_out_commit` binding), there is no "mid-move"
  git state to worry about — a commit's tree is one snapshot, not a
  sequence of file operations a checker could observe partially.

Sanity-checked against the real repository: every RFC number 093–100
currently resolves to exactly one file across the four lifecycle folders,
confirming the assumption the design rests on is currently true, not just
theoretically sound.

**No correction needed here.** This is the one claim where attacking it
harder didn't find anything, and I'd rather say that plainly than manufacture
a finding to seem thorough.

## Claim D — recommend "not required," with a caveat now covered by A2

**Should a `[gate_matrix_exceptions]` entry be required to have a
`[gate_owners]` entry? No.**

An exception is, by definition, a lane the manifest-driven byte-match
system does not govern — it's exempt from check 4 (command matching)
entirely, which is the only check ownership actually feeds. A
`[gate_owners]` entry for an exception would be bookkeeping with no
enforcement behind it. The existing (and unaffected) requirement that
every `[gate_matrix_exceptions]` entry carry a **reason** already captures
the "why does this exist and under whose authority" information in
practice — G12's real entry names its RFC context in prose ("RFC 093 M1a")
without needing a structured field to do it.

**The caveat: this recommendation does not substitute for Finding A2.**
Whether or not exceptions require an owner, they still need to be grounded
in a real lane row somewhere (A2's missing check). Requiring an owner
would not even fully close A2 on its own — an owned-but-ungrounded
exception (owner points at a real RFC, but that RFC's table has no such
lane) is a distinct, still-open failure mode unless A2's specific check is
added. Decide D on its own merits; add A2 regardless of how D is decided.

## Claim E — one more, found while attacking A

**The `[gate_owners]` table itself has no duplicate-key protection stated,
but TOML already provides it — worth saying explicitly rather than leaving
readers to know that.** §*Why ownership conflicts cannot occur* correctly
relies on "a TOML table cannot have duplicate keys" for **lane → owner**
conflicts. The same property applies to `[gate_lane_sources]` (**RFC
number → heading**) — two different headings for the same RFC number is
equally a parse error, not a runtime check, by the identical reasoning.
The design doesn't state this explicitly for `[gate_lane_sources]`, and
while it's a direct corollary of the same argument already made for
`[gate_owners]`, saying it once more costs a sentence and removes a
reader's need to re-derive it.

## What I checked and did not check

Read all five named subsections in full, plus condition 7 in
`scripts/check-gate-inputs.sh` (lines 236–376) and the current
`ci/gate-inputs.toml`, against each other directly rather than from
memory of an earlier pass. Did not read the rest of RFC 094 this round —
out of scope, per the request. Did not implement anything in
`check-gate-inputs.sh`, `ci/gate-inputs.toml`, or the RFC; the harness was
throwaway Python, standalone, removed after use, never touching the
tracked tree.

## Recommendation

Add the two checks from A1/A2, state the three implementation
requirements from B, decide D (I'd choose "not required"), and note the
`[gate_lane_sources]` corollary from E. None of this changes the manifest
shape, the migration path, or the claim that RFC 093 needs no amendment —
it tightens four checks into six and adds a few sentences. Small fix,
real gap: exactly the "costs a document edit now" case the request's
standing instruction described.

---

`rfcs/reviews/094-lane-ownership-design-review-2026-08-27.md`
