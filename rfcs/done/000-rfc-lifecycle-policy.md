# RFC 000 — RFC lifecycle policy

**Status.** Implemented
**Tracks.** Cross-cutting documentation policy. Not tied to any
single feature; applies to the RFC directory itself.
**Touches.** `rfcs/` folder structure, the index file at
`rfcs/README.md`, the Status field convention used inside each
RFC, any cross-references between RFCs, and optional companion
handoff documents under `rfcs/handoffs/`.

## sui-id adoption profile — moved to RFC 018

*Moved 2026-08-27.* This section carried sui-id's project-specific approval and
independence rules inside the generic policy, and had gone stale: it required "a
named independent design reviewer", the identity-based rule withdrawn on
2026-08-26 in favour of role independence (see `ROADMAP.md` §S1).

Project-specific content belongs in the project's profile. It now lives in
[RFC 018 — sui-id RFC lifecycle profile](./018-rfc-lifecycle-policy.md), with
its independence language corrected. This RFC remains the general policy; RFC
018 governs for sui-id where the two differ.

## Summary

This RFC defines a lifecycle for RFCs themselves: where they live,
how they move between states, and what each state means. It is
written to be portable — any project starting an `rfcs/` (or
similarly-named) directory can adopt this policy verbatim. The
recommendations are deliberately conservative: the smallest set of
folders that gives implementers a clear answer to "what should I
look at next?" without imposing process overhead that small teams
will route around.

The policy's central claim is that **completed RFCs are not
deleted**. They move to a fixed location and stay there as a
record of the design decisions, alternatives considered, and
open questions resolved.

This policy also defines a small convention for optional
implementation handoffs. Handoffs are companion execution
documents for RFCs; they do not have their own lifecycle state.
Their status is inherited from the related RFC. The reasoning,
anti-patterns, and adoption guidance are spelled out below.

## Why a written policy

A common pattern in young projects: an `rfcs/` folder accumulates
fifteen or twenty Markdown files in flat structure, all named
`NNN-slug.md`. Some are implemented; some are abandoned; some
are mid-review. New contributors cannot tell which is which
without reading each file's prose. The maintainer keeps the
state in their head.

Eventually one of three things happens, all bad:

1. The maintainer prunes the folder, deleting "obviously done"
   RFCs to clean up. The design rationale is lost. Six months
   later, someone proposes the same idea, and the discussion
   restarts from zero.
2. The folder grows to fifty files. Contributors give up on
   reading any of them.
3. A second informal status system emerges — issue labels,
   spreadsheets, project boards — that the RFC files
   themselves don't reflect, creating a drift problem.

This RFC averts all three by fixing the rules up front.

## Scope and applicability

This policy targets projects whose RFC directory has roughly
**5 to 100 RFCs** at any given time, maintained by **one to ten
core people**, with **occasional outside contributors**. Above
that scale, the policy still works but probably needs additional
machinery (review SLAs, automated state checks, a dedicated RFC
shepherd role) that is out of scope here.

Below that scale (a project with three RFCs total), this policy
is overkill — a single flat folder with a README is enough.
Adopt only when flat starts to hurt.

The policy makes no claim about *what* an RFC contains. Each
project decides whether RFCs are tightly templated or
free-form, whether they cover features only or also operational
decisions, and whether they double as Architecture Decision
Records. This RFC governs only their lifecycle and storage.

Some projects also maintain developer handoffs, PR plans, or QA
checklists that explain how to implement a particular RFC. Those
are not RFCs and should not introduce a parallel status system.
This policy gives them a small companion-document convention so
they remain findable without becoming a second lifecycle.

## Lifecycle states

An RFC is in exactly one of the following states at any time:

| State | Meaning |
|---|---|
| **Draft** | The author is still writing. Not ready for review by anyone but the author and immediate collaborators. |
| **Proposed** | Open for review and discussion. Implementer should *not* yet start work — the design may change. |
| **Accepted** | Design review is complete and approval is recorded. It is the only implementation-eligible state, but coding waits for repository-visible passing evidence for every implementation prerequisite. |
| **Implemented** | The work has shipped (in a release, on `main`, or wherever the project's stability marker lives). The RFC is now a historical record. |
| **Withdrawn** | The author or maintainer decided not to pursue this RFC. The work will not happen. |
| **Superseded** | A later RFC replaces this one. The replacement RFC's identifier is recorded in this RFC's Status field. |

For sui-id, Accepted is an authorization boundary rather than a progress label.
Only the folder plus matching repository-visible approval metadata authorizes
implementation.

## Folder layout

The sui-id structure is **five folders**, four of which hold RFCs:

```
rfcs/
  README.md           ← index; lists all RFCs by state
  proposed/           ← Proposed RFCs (open for review)
    NNN-slug.md
  accepted/           ← Accepted RFCs (design-approved; implementation-eligible)
    NNN-slug.md
  done/               ← Implemented RFCs
    NNN-slug.md
  archive/            ← Withdrawn or Superseded RFCs
    NNN-slug.md
```

An optional folder holds Drafts:

```
  draft/              ← (optional) Draft RFCs not yet in review
    NNN-slug.md
```

Most small projects do not need `draft/` — authors can write
in a personal branch or a gist until they're ready to open a
review. Add `draft/` only when multiple authors regularly
need a shared place to share drafts.

A project may also add an optional `handoffs/` folder for
implementation companion documents:

```
  handoffs/          ← optional; companion execution docs for RFCs
    NNN-slug/
      implementation-handoff.md
      task-breakdown-pr-plan.md
      acceptance-qa-checklist.md
      README.md      ← optional index / scope note for this RFC's handoffs
```

`handoffs/` is deliberately not split into `proposed/`, `accepted/`, `done/`,
or `archive/`. A handoff's status is inherited from the matching
RFC number. If `rfcs/proposed/057-slug.md` is Proposed, then
`rfcs/handoffs/057-slug/` is a proposed implementation companion.
If it moves to `accepted/`, the handoff becomes implementation-eligible with
it, subject to the RFC's implementation prerequisites.
If the RFC moves to `done/`, the handoff becomes historical with
it. If the RFC moves to `archive/`, the handoff follows that
meaning. Do not manage handoff status separately.

Each top-level RFC state folder corresponds 1-to-1 with a lifecycle state.
**The folder is the source of truth for the state.** A file's
location is what determines its state, not the Status field
inside the file (the file's Status field must be kept consistent
with the folder, but if the two ever disagree, the folder wins).

Movement between folders is the operation that changes an RFC's
state. To accept an RFC for implementation, record its approval metadata and
move it from `proposed/` to `accepted/` in the same change. To mark it
Implemented, move it from `accepted/` to `done/`. To withdraw or supersede it,
move it to `archive/`.

Handoffs do not move between state folders. They remain under
`rfcs/handoffs/NNN-slug/`; their lifecycle meaning is derived
from the RFC's current folder and Status field.

### Why sui-id uses the 5-folder variant

For organisations where the design and implementation roles
are clearly separated, a fifth folder makes the boundary
explicit:

```
rfcs/
  proposed/    ← under review
  accepted/    ← design approved; implementation eligible after prerequisites
  done/        ← shipped
  archive/     ← withdrawn or superseded
  draft/       ← (optional)
```

For sui-id, "design approved" is meaningfully distinct from "implementation
finished." `accepted/` may legitimately be empty; its value is the
unambiguous prohibition on implementing Proposed work.

## Companion handoffs

A handoff is an optional implementation companion to an RFC. It
is useful when an RFC is large enough that implementers need a
separate execution package: implementation notes, PR sequencing,
acceptance checks, QA cases, migration reminders, or release
risks.

A handoff should answer a different question from the RFC:

- the RFC records **what decision was made and why**;
- the handoff records **how to implement and verify it safely**.

Handoffs must not override RFC decisions. If handoff work
uncovers a design conflict, update the RFC first, then update
the handoff to match. The RFC remains the authority for design
and lifecycle status.

A conventional handoff directory is:

```text
rfcs/handoffs/NNN-slug/
  implementation-handoff.md
  task-breakdown-pr-plan.md
  acceptance-qa-checklist.md
  README.md                 ← optional
```

These filenames are recommendations, not mandatory policy. Small
RFCs often need no handoff. Large multi-PR RFCs benefit from one.
Projects should avoid putting rough chat transcripts, obsolete
review notes, or every intermediate discussion into
`rfcs/handoffs/`; only current, reviewed, implementation-useful
companion documents belong there.

## Status field inside each RFC

Each RFC carries a `Status` field at the top, alongside other
metadata. The exact format is up to each project; one common
shape:

```markdown
# RFC NNN — Title

**Status.** Proposed
**Tracks.** What this addresses.
**Touches.** Where the work lands.
```

The Status field's value mirrors the folder. When an RFC moves
between folders, the Status field updates in the same commit.
For Implemented RFCs, the Status field carries the version or
release tag in which the work shipped:

```markdown
**Status.** Implemented (v1.4.0)
```

For Superseded RFCs, the field names the replacement:

```markdown
**Status.** Superseded by RFC 042
```

For Withdrawn RFCs, the field carries a one-line reason:

```markdown
**Status.** Withdrawn — overlapped with RFC 035; merged there.
```

Two reasons to keep this redundancy with the folder:

1. **Self-contained files.** A reader who opens an RFC by URL
   without seeing the folder context can still tell the state.
2. **Version-control history.** `git log -p path/to/rfc.md`
   shows state transitions inline, even if the file moves
   between folders (some VCS tools track moves better than
   others).

## Naming and numbering

RFCs are numbered sequentially from `001`. Numbers are assigned
when the file is first created — not when it ships, not when
it's accepted. **Numbers are stable forever**: a file does not
get renumbered when it moves between folders, even if its
priority or order changes.

The filename is `NNN-slug.md` where `NNN` is the zero-padded
number and `slug` is a short, lowercase, hyphen-separated
description. The slug is for human readers; the number is for
unambiguous reference.

```
001-feature-flags.md
015-deprecate-old-api.md
142-storage-backend-abstraction.md
```

Three-digit numbering covers the first 999 RFCs, which is more
than most projects ever reach. If the project does cross 999,
switch to four digits prospectively (new RFCs use `NNNN`); do
not retroactively renumber existing files.

**Numbers are never reused.** If RFC 005 is withdrawn, the
number stays in `archive/` and the next new RFC is 006.
Renumbering would invalidate cross-references, audit logs, and
any external link to the file.

## Cross-references between RFCs

When one RFC references another, use a relative path that
reflects the target's current folder:

```markdown
See [RFC 010](../done/010-revoke-tokens.md) for the prior work.
```

This means cross-references break when an RFC moves between
folders. That is acceptable — it's a small, finite, mechanical
fix that surfaces during review and is easy to grep for.
Tooling can help (see [§ Optional CI invariants](#optional-ci-invariants)
below), but for projects without CI on RFC files, periodic
manual sweeps suffice.

A common convention to make this less painful: when an RFC
moves to `done/` or `archive/`, the maintainer also runs a
quick `grep -l "<NNN>-<slug>.md" rfcs/` to find inbound
references and updates them in the same commit. The cost is
seconds per move; the alternative — broken links accumulating
silently — is worse.

If your project's review tool renders relative links (GitHub,
GitLab, sourcehut all do), broken links become visible in the
preview, which gives a second line of defence.

If a handoff exists for an RFC, the RFC may link to it with a
relative path such as `../handoffs/057-slug/README.md`, and the
handoff should link back to the RFC. These links are references,
not lifecycle state. Moving the RFC between `proposed/`, `done/`,
and `archive/` requires the same link sweep described above.

## Review and transitions

The transitions between states are:

```
[author writes]──▶ Draft? ──▶ Proposed ──▶ Accepted ──▶ Implemented
                                ▲          │                │
                                └── material ─┘                ▼
                                    change                 (lives in done/)
                                └────┬─────┘
                                     ▼
                               Withdrawn /
                               Superseded
                                     ▼
                            (lives in archive/)
```

State transitions are operations performed by the maintainer
(or whoever has commit authority on the RFC directory). The
operations:

- **Open.** New file in `proposed/` (or `draft/` if used).
  Triggered by an author opening a pull request adding the
  file.
- **Accept.** Record approval metadata, update Status to Accepted, move from
  `proposed/` to `accepted/`, and update the index and inbound links in the same
  change. Accepted is necessary but not sufficient for coding; implementation
  begins only after repository-visible evidence shows all implementation
  prerequisites pass.
- **Return for review.** A material change to security invariants, public
  behavior, scope, or prerequisites atomically moves an Accepted RFC back to
  `proposed/`, removes active approval metadata, updates the index/links, and
  preserves the old decision in history or a superseded-review note.
- **Ship.** After implementation and closure prerequisites pass, move from
  `accepted/` to `done/` and record the shipping version. Security-sensitive
  RFCs also record the independent closure reviewer, date, and durable evidence.
- **Withdraw.** The author or maintainer decides not to pursue
  the RFC. Move to `archive/` with Status updated and a brief
  reason added in the file.
- **Supersede.** A new RFC takes over the design space of an
  older one. Move the older RFC to `archive/`, update its
  Status to `Superseded by RFC NNN`, and add a reciprocal note
  in the new RFC.

There is no "rejected" state distinct from "withdrawn". An RFC
that the maintainer declines to accept is moved to `archive/`
with a reason — the file is preserved as evidence that the
discussion happened.

### Granularity of transitions

A single RFC does not need to enumerate all its sub-features
to qualify as Implemented. Partial implementation is fine if
the partial work captures the RFC's main design decision; any
deferred work either gets a follow-up RFC or is logged in the
RFC's Status section as an explicit "deferred" note.

This is a judgement call. The principle: **don't keep an RFC
in `proposed/` indefinitely just because one open question
remains**. Move it to `done/` when the design has shipped, and
record what didn't make it.

## README integrity

The `rfcs/README.md` file serves as the index. It should:

1. List all RFCs across all folders, grouped by state or
   priority (whichever is more useful for the project).
2. Use relative links that reflect each RFC's current folder.
3. Be updated in the same commit that moves an RFC between
   folders.
4. Optionally indicate that a handoff exists for RFCs that have
   one, without treating the handoff as a separate lifecycle
   item.

A typical structure:

```markdown
# Project RFCs

## Proposed
| ID | Title | Priority |
|----|-------|----------|
| 042 | [Feature flags](./proposed/042-feature-flags.md) | High |
| 047 | [Caching layer](./proposed/047-caching.md) | Medium |

## Implemented
| ID | Title | Shipped in |
|----|-------|------------|
| 010 | [Revoke tokens](./done/010-revoke-tokens.md) | v1.4.0 |
| 015 | [Deprecate API](./done/015-deprecate-old-api.md) | v1.5.0 |

## Archive
| ID | Title | Reason |
|----|-------|--------|
| 023 | [Multi-region](./archive/023-multi-region.md) | Withdrawn |
| 035 | [Old caching](./archive/035-old-caching.md) | Superseded by RFC 047 |
```

Some projects prefer prose to tables; either works. The point
is that the index reflects every RFC's current state and
location, and the index is what new contributors read first.

## Optional CI invariants

For projects with CI on the RFC directory, the following
invariants are worth checking:

- Every file under `rfcs/<state>/` has a Status field whose
  value matches the folder.
- Every relative link inside an RFC resolves to an existing
  file.
- No RFC number is duplicated across folders.
- Every RFC listed in `rfcs/README.md` exists at the linked
  path; every RFC under `rfcs/` is listed in `README.md`.
- Filenames match the `NNN-slug.md` pattern and the slug
  matches the title (loosely).
- Every `rfcs/handoffs/NNN-slug/` directory, if handoffs are
  used, corresponds to an existing RFC number.
- Handoff directories do not duplicate lifecycle state with
  their own `proposed/`, `done/`, or `archive/` subfolders.

A simple script in `scripts/check-rfcs.sh` or
`xtask check-rfcs` can run these checks. None of them need
sophisticated parsing — `grep`, `find`, and basic shell
suffice.

For projects without CI on the RFC directory, these checks
are still useful as a periodic manual hygiene pass. Don't
build elaborate tooling before the project's scale demands it.

## Adoption guidance for new projects

If you're starting an `rfcs/` directory from scratch, the
minimum viable adoption of this policy is:

1. Create `rfcs/proposed/`, `rfcs/accepted/`, `rfcs/done/`, `rfcs/archive/`.
2. Add `rfcs/README.md` with a state-grouped index.
3. Adopt the `NNN-slug.md` naming and start at `001`.
4. Write the first RFC. Put it in `proposed/`.
5. When review approves it, record approval and move it to `accepted/`.
6. When the work ships, move it to `done/` with a Status field carrying the
   release tag.

If an RFC is large enough to need execution guidance, add an
optional companion directory under `rfcs/handoffs/NNN-slug/`. Do
not add a separate handoff status folder; the RFC's state is
enough.

That's the entire policy in six steps, plus optional handoffs
when they are genuinely useful. The other sections of
this RFC exist to handle edge cases as the directory grows;
ignore them until you hit the relevant case.

If you're adopting this policy for an *existing* RFC directory:

1. Audit each existing file. Decide its state.
2. Move it to the corresponding folder.
3. Add or update the Status field in each file.
4. Rewrite cross-references with the new paths.
5. Rebuild `rfcs/README.md` to reflect the new structure.
6. If handoffs already exist, move only reviewed, current,
   implementation-useful handoffs under `rfcs/handoffs/NNN-slug/`
   and let their state inherit from the matching RFC.

The migration is mechanical but tedious. Schedule it as a
single dedicated change rather than spreading it across
unrelated commits.

## Anti-patterns

Patterns that look reasonable but cause long-term harm:

### Deleting completed RFCs to "clean up"

The most common mistake. The reasoning sounds correct: "the
RFC is implemented, the code and CHANGELOG capture the
result, the design document is now redundant." It isn't.
RFCs capture the *why* — alternatives considered, trade-offs
weighed, open questions resolved. Code captures the *what*.
The two are different artifacts; both are needed.

When an RFC is deleted, future contributors see the current
code and have no record of why it isn't different. They
re-derive the design space from scratch, often missing the
constraints that drove the original choice. Months later,
someone proposes the same alternative that the original RFC
already considered and rejected, and the discussion repeats.

The fix: never delete. Move to `done/` and leave it there.

### Renumbering RFCs during reorganisation

Tempting when migrating an old flat directory: renumber to
fill gaps from withdrawn RFCs, or renumber by priority order
in the new folders. Don't. External references — issue
trackers, commit messages, Slack history, design-review
documents — all reference RFC numbers. Renumbering breaks
every one of those references silently.

The numbering is permanent. Withdrawn numbers stay withdrawn.

### Treating `accepted/` as an informal label

Moving a file to `accepted/` without approval metadata, named ownership, and
prerequisite review recreates a parallel status system. Move and metadata/index
updates are one change. A material design change returns the file to
`proposed/`; it does not retain stale approval.

### Letting cross-references rot

When an RFC moves between folders, inbound references break.
If the project doesn't fix them in the same commit, broken
references accumulate. After a few moves, the index becomes
unreliable and contributors stop trusting links.

The fix: a one-line `grep -l 'NNN-slug.md' rfcs/` before
every move, plus updating every match. Or CI enforcement.
Either works; doing nothing does not.

### Status fields that lie

If an RFC's Status field says `Proposed` but the file lives
in `done/`, contributors don't know which is correct. The
folder is authoritative by this policy, but a misleading
Status field still causes friction every time someone reads
the file directly (without seeing the folder context).

The fix: update Status in the same commit that moves the
file. CI can enforce; manual review can catch.

### Silent withdrawal

An RFC that's been abandoned but not formally withdrawn
sits in `proposed/` indefinitely. Contributors waste effort
reviewing it; the maintainer's unspoken "I'm not going to
do this" is invisible.

The fix: when you decide not to pursue an RFC, move it to
`archive/` with a one-line reason. Even "didn't pan out;
priorities shifted" is enough. Silence is worse.

### Turning handoffs into a second RFC lifecycle

Implementation handoffs can be useful, but they become harmful
when they gain their own parallel status system. A handoff in
`handoffs/proposed/` beside an RFC in `done/` forces readers to
ask which status is authoritative. A handoff that says "accepted"
while the RFC says `Proposed` creates the same confusion as a
lying Status field.

The fix: keep handoffs under `rfcs/handoffs/NNN-slug/` and let
the matching RFC's folder and Status field define their state.
Handoffs are companions, not separate lifecycle artifacts.

### Letting handoffs override RFC decisions

A handoff is closer to an implementation guide than a design
record. If it changes the architecture, narrows scope, or adds a
new acceptance requirement that contradicts the RFC, future
maintainers have to read two documents and guess which one wins.

The fix: when a handoff discovers a design problem, patch or
supersede the RFC first. Then update the handoff so it describes
execution of the current RFC, not a competing design.

## Self-application

This RFC describes its own placement: it is itself an RFC
governed by the policy it defines, and it lives in
`rfcs/done/` because it is implemented (the policy is now in
effect for this project's RFC directory).

The transition that landed this RFC is the simultaneous
adoption of the policy and the migration of every existing
RFC into the new folder structure. Both happened in the same
release. This is the recommended adoption pattern for
existing directories: combine the policy's introduction with
the migration into a single, atomic change.

## Open questions

None at time of acceptance. Future refinements (review SLAs,
automated state-machine checks, integration with project
management tools) will, if needed, land as follow-up RFCs
referencing this one.
