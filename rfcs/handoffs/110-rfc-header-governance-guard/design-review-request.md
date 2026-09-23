# RFC 110 — independent design review request

**RFC.** [RFC 110 — An RFC header may not legislate](../../proposed/110-rfc-header-governance-guard.md). Proposed.
**Reviewer.** Mid-capability model, implementation role. It authored neither the
RFC nor its handoff.
**Route.** Owner decision of 2026-08-26 (`ROADMAP.md` §S1): a design is reviewed
by the role that must build against it.
**Why now.** `@nabbisen` accepted this RFC on 2026-09-24. RFC 000 requires a
named independent design reviewer for a security-sensitive RFC, and G11 refuses
an Accepted RFC whose `Independent design review` field has no durable
reference. This RFC is item 3 of release cycle A.
**Baseline.** `9287e0d` or later.
**Scope.** Read-only. Change no code and no RFC text. Report findings.

## What this guards, and why it is not hypothetical

Twice, a governance rule was written into RFC headers, attributed upward to an
authority that does not contain it, and left unchecked.

- **2026-07-30.** A vendor-independence rule went into four headers attributed
  to an "Owner ruling, 2026-07-28" that cannot be evidenced. It blocked seven
  RFCs for four weeks. Withdrawn 2026-08-26 (`ROADMAP.md` §S1).
- **2026-09-22.** Eleven headers (093–103) were found asserting that a reviewer
  "must not have authored, implemented, or previously approved this RFC" —
  a bar RFC 000 does not contain — nine attributing it to RFC 000 and two to the
  disposed RFC 018. Removed in `363e75b`, 73 lines, nothing added.

Deleting the clauses fixed the instance. This RFC fixes the mechanic.

## 1. Is condition 14 implementable without false positives?

The rule: an RFC **header** may not state a rule about who may review; it may
**record** who reviewed something.

1. **Is the header block well-defined?** G11 already parses "the title line up
   to but excluding the first level-2 heading". Confirm that boundary is exact
   for every RFC in the tree, including RFCs whose first `## ` is late or
   absent, and RFCs with fenced blocks or template examples in the header
   region. `rfcs/done/000` and `rfcs/archive/018` both embed example metadata
   as prose — confirm they are handled.
2. **Run the five literal phrases over every RFC header today** — `must not
   have authored`, `Role independence`, `independence means` (any case),
   `vendor is not a criterion`, `cannot be the sole approver`. How many hits?
   **The answer should be zero.** If it is not, name each, because the RFC
   claims the gate is green on the tree it lands in without editing any RFC to
   make it so.
3. **Run them over every RFC *body*.** How many hits, and would any of them be
   caught by a boundary bug? A body hit that the gate catches is a false
   positive and a reason to distrust the gate.
4. **Is the field-label rule right?** The RFC also rejects any header field
   labelled `Independent security and <anything> reviewer`. Confirm no
   legitimate use exists, given that `Independent design review` and
   `Accountable owner and approver` remain required and carry the reviewer
   facts.
5. **The exemption.** `**Reviewer-rule exemption.**` disables condition 14 for
   that RFC when its value cites the owner's dated approval. Is "cites a dated
   owner approval" checkable, or does it degrade into a regex that any text
   satisfies? **If it cannot be checked meaningfully, say so** — an exemption
   that anyone can write is a hole, and it would be better to have no exemption
   and require a policy change instead.

## 2. Is condition 15 implementable?

The rule: no RFC header may cite an archived RFC as authority.

6. **How is "RFC NNN" resolved to a folder?** G11 already resolves an RFC
   number to exactly one `NNN-*.md` across the four lifecycle folders. Confirm
   that machinery is reusable here.
7. **What forms must be caught?** `RFC 018`, `RFC-018`, a Markdown link to
   `../archive/018-*.md`, a bare `018`. Say which you would catch and which you
   would not, and why the ones you skip are safe to skip.
8. **False positives.** Is there a legitimate reason for a header to name an
   archived RFC — a supersession note, a "replaces RFC NNN" line? If so, the
   condition needs a carve-out; say which, precisely.

## 3. Sequencing and hosting

9. **Does this belong in G11?** G11's docstring says items 12 and 13 are not
   RFC 093's and that "G11 hosts them because this is the RFC-structure gate;
   RFC 000 remains their source." Conditions 14 and 15 follow the same pattern
   — confirm that is right, and that adding them does not require amending RFC
   093's closed contract or changing the G11 command in
   `ci/gate-inputs.toml` (which would pull in A3.4's lane-agreement check).
10. **Nine negative self-tests** are specified in the handoff. Are they the
    right nine? Name any branch they miss, especially around the header
    boundary and the exemption.

## 4. One open question the owner has not ruled on

11. **Is this guard aimed widely enough?** The architect raised, and
    `@nabbisen` has not ruled on, a wider finding: the recurring mechanic is not
    confined to RFC headers. Seven distinct dated owner-attributions exist
    across 22 tracked files — `Owner ruling`, `Owner authorization`, `Owner
    decision` — and **three are already known bad**: 2026-07-28 (withdrawn),
    2026-09-09 (recorded in RFC 098, contradicting the decision of 2026-08-26),
    and 2026-09-10 (which created the `roadmap/` directory; the owner states he
    made no such ruling). None can be verified, because every commit in this
    repository carries the owner's git identity, including every commit an agent
    makes.

    The wider guard would be: a single ledger recording the owner's decisions in
    his own words, and a gate rejecting any claim of an owner decision anywhere
    in the tree that does not cite a ledger entry.

    **Give your view: is the narrow, header-scoped guard in this RFC worth
    building on its own, or is it a partial fix that will need replacing?** Say
    which, with reasons. The scope decision is `@nabbisen`'s; your answer is an
    input to it.

## What to return

A review-request package under `.git-exclude/review-requests/`, containing:
- the measured answers to items 2 and 3 (hit counts, header and body);
- findings ranked blocker / high / medium / low;
- your answers to items 1, 4–10, each citing what you read;
- your view on item 11, marked as a view.

Do not implement anything. This RFC is Proposed; implementation is not
authorized until it is Accepted, which this review enables.
