# RFC 018 — RFC lifecycle policy

**Status.** Implemented
**Tracks.** Cross-cutting documentation policy. Not tied to any
single feature; applies to the RFC directory itself.
**Touches.** `rfcs/` folder structure, the index file at
`rfcs/README.md`, the Status field convention used inside each
RFC, and any cross-references between RFCs.

## Summary

This RFC defines a lifecycle for RFCs themselves: where they live,
how they move between states, what each state means, and which state
authorizes implementation. sui-id originally used the four-folder
variant. On 2026-07-16 it adopted the five-folder variant because
design approval, implementation, and independent security review are
now meaningfully separate responsibilities.

The policy's central claim is that **completed RFCs are not
deleted**. They move to a fixed location and stay there as a
record of the design decisions, alternatives considered, and
open questions resolved. The reasoning, anti-patterns, and
adoption guidance are spelled out below.

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

## Lifecycle states

An RFC is in exactly one of the following states at any time:

| State | Meaning |
|---|---|
| **Draft** | The author is still writing. Not ready for review by anyone but the author and immediate collaborators. |
| **Proposed** | Open for review and discussion. Implementer should *not* yet start work — the design may change. |
| **Accepted** | Design review is complete and approval is recorded. This is the only implementation-eligible state, but coding remains prohibited until every implementation prerequisite has repository-visible passing evidence. |
| **Implemented** | The work has shipped (in a release, on `main`, or wherever the project's stability marker lives). The RFC is now a historical record. |
| **Withdrawn** | The author or maintainer decided not to pursue this RFC. The work will not happen. |
| **Superseded** | A later RFC replaces this one. The replacement RFC's identifier is recorded in this RFC's Status field. |

The Accepted state is a security and authorization boundary, not a progress
label. Chat approval, an external project board, or roadmap wording cannot
substitute for the file's location and approval record.

## Folder layout

The project structure is **five folders**, four of which hold RFCs:

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

Each top-level folder corresponds 1-to-1 with a lifecycle state.
**The folder is the source of truth for the state.** A file's
location is what determines its state, not the Status field
inside the file (the file's Status field must be kept consistent
with the folder, but if the two ever disagree, the folder wins).

Movement between folders is the operation that changes an RFC's
state. To accept an RFC for implementation, record its approval
metadata and move it from `proposed/` to `accepted/` in the same
change. To mark it Implemented, move it from `accepted/` to
`done/`. To withdraw or supersede it, move it to `archive/`.

### Why sui-id uses the 5-folder variant

The explicit Accepted boundary is required because sui-id now separates
project ownership, architecture/security approval, implementation, and
closure-evidence review:

```
rfcs/
  proposed/    ← under review
  accepted/    ← design approved; implementation eligible after prerequisites
  done/        ← shipped
  archive/     ← withdrawn or superseded
  draft/       ← (optional)
```

`accepted/` may legitimately be empty. Its value is the unambiguous rule that
no Proposed design authorizes code work.

## Status field inside each RFC

Each RFC carries a `Status` field at the top, alongside other
metadata. The exact format is up to each project; one common
shape:

```markdown
# RFC NNN — Title

**Status.** Proposed
**Security review.** Required | Not required — reason approved by NAME
**Design prerequisites.** None
**Implementation prerequisites.** None
**Closure prerequisites.** None
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

For Accepted RFCs, the file also carries the repository-visible approval
record:

```markdown
**Status.** Accepted
**Security review.** Required
**Accepted on.** 2026-07-16
**Approved by.** Project owner or delegated RFC approver
**Independent design review.** Reviewing role, what it checked, and durable
review reference
**Implementation owner.** Named person
```

Every new RFC classifies security review as `Required` or `Not required`. A
Not-required classification includes a concrete reason and the name of the
approver who accepted that classification. RFCs that change authentication,
authorization, secrets, tokens, sessions, audit guarantees, external trust
boundaries, security-relevant storage/transactions, or assurance controls are
Required. All security-remediation RFCs 093–099 are Required; a narrower
classification is an owner decision with a recorded reason. It is not a
reviewer's decision — a rule about who may review cannot be relaxed by the
reviewer it governs.

When security review is Required, `Independent design review` must record the
reviewing role, what that role checked, and a durable review reference; `N/A`
is prohibited.

Independence here means **role independence**: the reviewer did not author the
artifact, implement it, or previously approve it. It is a property of which
role performed the review — not of which vendor, model, or organization the
reviewer belongs to. A review by a party sharing the author's vendor is a valid
review. A review by the author is not, whatever their vendor.

Review routes by role:

- a **design** is reviewed by the role that must build against it, for
  implementability and for gaps it would hit;
- an **implementation** is reviewed by the role that specified it, against the
  specification and by executing its evidence;
- **rules, scope and schedule** are proposed by the architect and decided by
  the owner. The architect does not record its own proposal as an owner
  decision.

The implementer may contribute to design and evidence, but cannot be the sole
approver of the security invariants or closure evidence.

Where no role other than the author can perform a given review, the review is
neither waived nor fabricated. The field records what was verified by
execution, and states plainly what remains an unreviewed design judgment; the
owner accepts or rejects with that gap in view. A field that reads as a
completed independent review when none occurred is a defect of the same class
this policy exists to prevent.

The three prerequisite fields have distinct meanings:

- **Design prerequisites** must be resolved before the RFC can become Accepted.
- **Implementation prerequisites** may permit design acceptance but must be
  complete before coding begins.
- **Closure prerequisites** must be complete before the RFC moves to `done/`.

This distinction is mandatory for new RFCs. A dependency edge must not be
treated as blocking all three stages unless the RFC says so explicitly.

For a security-sensitive RFC to move from Accepted to Implemented, the RFC
must add this repository-visible closure record:

```markdown
**Closure reviewed on.** 2026-08-28
**Closure approved by.** Reviewing role, or the owner where no other role qualified
**Closure evidence.** ../path/to/durable-evidence-or-review.md
```

The closure approver cannot be the sole implementer. The evidence reference
must resolve to a durable, repository-relative artifact or review record and
must identify the observed results used for closure. A release version alone
is not closure evidence.

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

## Review and transitions

The transitions between states are:

```
[author writes]──▶ Draft? ──▶ Proposed ──▶ Accepted ──▶ Implemented
                                ▲          │                │
                                └── material ─┘                ▼
                                    change                 (lives in done/)
                                │          │                forever
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
- **Accept.** Review is complete. Record the acceptance date, approver,
  security classification, independent design review where required, and
  implementation owner; update Status to Accepted; move from `proposed/` to
  `accepted/`; update the index and inbound links in the same change. Accepted
  is necessary but not sufficient for coding: implementation may start only
  after every implementation prerequisite has repository-visible passing
  evidence.
- **Return for review.** If an Accepted RFC changes materially in security
  invariants, public behavior, scope, or prerequisites, update Status to
  Proposed, remove active-looking acceptance metadata, move it back to
  `proposed/`, update the index, and repair inbound links in the same change
  before further implementation. Preserve the prior decision in version-control
  history or an explicit superseded-review note.
- **Ship.** RFC is implemented and every closure prerequisite has passed;
  for security-sensitive work, record the closure-review date, independent
  closure approver, and durable closure evidence; then move it from
  `accepted/` to `done/` and update Status with the release tag in the same
  change that records the shipped implementation.
- **Withdraw.** The author or maintainer decides not to pursue
  a Proposed or Accepted RFC. Move it to `archive/` with Status updated and a
  brief reason added in the file.
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

This is a judgement call. The principle: do not mark a partially implemented
RFC done if an unfulfilled closure prerequisite or security invariant remains.
Move it to `done/` only when the core design has shipped and record any
explicitly non-blocking deferred work or follow-up RFC.

## README integrity

The `rfcs/README.md` file serves as the index. It should:

1. List all RFCs across all folders, grouped by state or
   priority (whichever is more useful for the project).
2. Use relative links that reflect each RFC's current folder.
3. Be updated in the same commit that moves an RFC between
   folders.

A typical structure:

```markdown
# Project RFCs

## Proposed
| ID | Title | Priority |
|----|-------|----------|
| 042 | [Feature flags](./proposed/042-feature-flags.md) | High |
| 047 | [Caching layer](./proposed/047-caching.md) | Medium |

## Accepted
| ID | Title | Implementation owner |
|----|-------|----------------------|
| 050 | [Session hardening](./accepted/050-session-hardening.md) | Alice |

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
- Every Accepted RFC records its acceptance date, approver,
  implementation owner, security-review classification, and required
  independent design review.
- Accepted RFCs use distinct design, implementation, and closure
  prerequisite fields.
- Every Implemented security-sensitive RFC records its closure-review date,
  independent closure approver, and resolvable durable evidence reference.

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

1. Create `rfcs/proposed/`, `rfcs/accepted/`, `rfcs/done/`,
   `rfcs/archive/`.
2. Add `rfcs/README.md` with a state-grouped index.
3. Adopt the `NNN-slug.md` naming and start at `001`.
4. Write the first RFC. Put it in `proposed/`.
5. When review approves it, record approval and move it to `accepted/`.
6. When the work ships, move it to `done/` with a Status field carrying the
   release tag.

That's the entire policy in six steps. The other sections of
this RFC exist to handle edge cases as the directory grows;
ignore them until you hit the relevant case.

If you're adopting this policy for an *existing* RFC directory:

1. Audit each existing file. Decide its state.
2. Move it to the corresponding folder.
3. Add or update the Status field in each file.
4. Rewrite cross-references with the new paths.
5. Rebuild `rfcs/README.md` to reflect the new structure.
6. For Accepted RFCs, add the approval and prerequisite metadata required by
   this policy.

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
prerequisite review recreates the parallel-status ambiguity this policy is
meant to remove. The fix is to perform the folder move and metadata/index/link
updates atomically. If a material design change follows, return the RFC to
`proposed/`; do not leave an obsolete approval attached to a changed design.

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

## Self-application

This RFC describes its own placement: it is itself an RFC governed by the
policy it defines, and it lives in `rfcs/done/` because the policy is in effect
for this project.

The original v0.29.5 transition introduced the four-folder policy and migrated
the existing RFC set in one release. The 2026-07-16 amendment adds an empty
`accepted/` state and the approval/closure rules above; it does not reclassify
or move any existing Proposed, Implemented, Withdrawn, or Superseded RFC.

The 2026-08-26 amendment redefines what `Independent design review` must record
and how reviews route between roles. It does not change any folder, state
transition, or existing classification, and no RFC is reclassified or moved by
it. Its cause is recorded in `ROADMAP.md` §S1: the previous rule defined
independence by vendor, was written by the architect role and attributed to an
owner ruling that cannot be evidenced, and blocked seven RFCs for four weeks.
The replacement defines independence as a property of the reviewing role and
requires that an unreviewable judgment be recorded as such rather than labelled
as a completed independent review.

## Open questions

None at time of acceptance. Future refinements (review SLAs,
automated state-machine checks, integration with project
management tools) will, if needed, land as follow-up RFCs
referencing this one.
