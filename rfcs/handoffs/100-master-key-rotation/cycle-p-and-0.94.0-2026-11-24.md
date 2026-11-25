# Cycle P and `0.94.0` — scoping it carefully

> ## Decided 2026-11-24, on §4's three questions
> `@nabbisen`'s answers, verbatim:
> 1. *"Two releases, M2c first (Recommended)"*: `0.94.0` closes M2c, and
>    `0.95.0` is the console, with RFC 101 and RFC 150 on it (cycle Q).
> 2. *"Accept the order (Recommended)"*: P1–P6, as §2's table gives them.
> 3. *"Yes, both (Recommended)"*: the owner is M2c's independent closure
>    reviewer, and the release-binary drill (§3) joins the evidence.
>
> ## Reordered 2026-11-24, before P1 was dispatched
> Writing P1, the architect found the accepted order would change the live
> rotation twice. Stage 5 would join the new preparation to the old
> finish, where the backup's custody choice has nowhere to be recorded,
> and stage 6 would rebuild it. `@nabbisen`: *"Reorder: database half first
> (Recommended)"*. The RFC 100 rows of §2 are now:
> - **P1**, the database half, built as modules the live rotation does not
>   use yet: the `master_key_rotation` row; K07 writing it pending, with
>   the custody choice; K05 completing it; and recovery rows 8–10;
> - **P2**, the live rotation rewritten **once**, to the whole protocol:
>   the liveness lock, the custody flags, the journal, publication, K07,
>   activation and K05;
> - **P3**, stage 7, unchanged.
>
> Every crash point then has its recovery before rotation can reach it.
>
> ## Progress
> - **P1 landed 2026-11-24** (`c3b8920`): [dispatch](0.94.0-p1-the-database-half-2026-11-24.md).
> - **P2 landed 2026-11-24** (`59ff4c9`): [dispatch](0.94.0-p2-the-rotation-rewritten-once-2026-11-24.md).
>   The review found that a second rotation, with the last backup still
>   kept, failed part-way, after which every start refused. Now refused
>   before anything is created.
> - **P3 landed 2026-11-25** (`2660276`, CI 28/28): [dispatch](0.94.0-p3-custody-and-the-operators-word-2026-11-24.md).
>   Custody and the operator's guide. Stage 7's warning is replaced by the
>   recovery procedure.
> - **The release-binary drill ran 2026-11-25**: [record](m2c-release-binary-drill-2026-11-25.md).
>   Three abort points; the shipped binary recovered each as the table
>   requires.
> - **P4, M2c's closure assessment, written 2026-11-25**: [assessment](m2c-closure-assessment-2026-11-25.md).
>
> - **P4 accepted 2026-11-25, on a condition.** The philosophy check found
>   both halves unmet. **P7** is the fix
>   ([dispatch](0.94.0-p7-what-the-philosophy-check-found-2026-11-25.md)),
>   and M2c closes when it lands.
> - **P7 landed 2026-11-25** (`070e7f7`, CI `45577018318` 28/28). Every
>   sui-id process now holds the rotation lock shared, and a rotation
>   refuses beside one, before its summary. The operator's words are
>   fixed. **M2c CLOSED 2026-11-25.**
>
> - **P5 and P6 dispatched 2026-11-25**: [P5](../151-password-policy-can-be-made-stricter/0.94.0-p5-the-minimum-can-be-raised-2026-11-25.md)
>   (RFC 151) and [P6](../097-threat-model/0.94.0-p6-the-inventory-and-its-gate-2026-11-25.md)
>   (RFC 097 stage 1). They are independent; P5 first.
>
> ## Whose turn it is: **the implementation role's**, on P5, then P6.

**Proposed.** 2026-11-24, by the architect, after `0.93.1` was published
(`7d6d250`).

## 1 — what cycle P holds today, measured

**As authorized** (ROADMAP, *Release cycles*, P; authorized 2026-11-15,
re-scoped 2026-11-16):

| Work | Status | Size, measured from its RFC |
|---|---|---|
| **RFC 100 stages 5–7**, then **M2c's independent closure review** | Accepted; stages 0–4 landed | Three stages, each a full dispatch. Stage 6 begins with recovery rows 8–10, moved there from stage 4 |
| **RFC 149**, the console follows the operator | Accepted 2026-11-15 | Six decisions: navigation by task (a sidebar), a first-run path, help in three layers, forms that offer known choices, denser tables, and the palette (already done) |
| **RFC 150**, an administrator can edit a user | Accepted; its username question answered 2026-11-16 | Display name and language, a username rename (Class A), and the email, through RFC 101 only |
| **RFC 101**, email verification and change | Accepted 2026-11-16; open questions settled (§6.2's event names are an implementation detail) | Seven design points: tokens, a pending address, both addresses told, step-up, Class-A writes |
| **RFC 151**, a stricter password policy | Accepted 2026-11-16 | One setting, one audited command, the minimum stated wherever a password is chosen |

**Added since, by the owner's decisions** (his words, verbatim):
- **RFC 097 stage 1**, the inventory contract and its gate. The method
  accepted 2026-11-23 placed it in cycle P (§7): *"Your recommendation is
  accepted."*
- **The RFC 148/107 closure assessment**, 2026-11-23: *"Cycle P
  (Recommended)"*.
- **Into RFC 149's build**, 2026-11-22: the footer, *"Cycle P, with the
  chrome redesign (Recommended)"*, and a short label for the DNS deny
  reason. They sit with the items carried 2026-11-16: C01's
  consent-policy attribute, `.kv-list`, the dead `page_get`, and the top
  bar becoming the sidebar.

**That is about twice cycle O.** O shipped seven builds (O1–O7), and
four problems were found only at the release:
- two advisories;
- a packaging defect, which stopped `0.93.0` after five of six crates;
- a race in the release check.
One release carrying both M2c's closure and a console rebuild would hold
**two unrelated risks** in one tag: a key-file protocol, and every page.
Each would delay the other.

## 2 — the recommendation: two releases, M2c first

**`0.94.0` — M2c closes.**

| # | Work | Why here |
|---|---|---|
| P1 | **RFC 100 stage 5**: preparation through the journal | Stage 4's recovery already covers what it writes, as the stage rules require. It carries stage 4's three items: one path derivation (done), syncs on both directories for cross-directory publication, and a typed `review_at`. **Most important, the liveness lock**: a running rotation holds an exclusive `flock` on the workspace, and recovery takes it non-blocking. Without it, recovery could delete the key a running rotation is about to reseal under |
| P2 | **RFC 100 stage 6**: recovery rows 8–10 **first**, then K07's pending row, activation, and K05, each durable | Recovery lands before what it recovers, inside the stage |
| P3 | **RFC 100 stage 7**: custody, the operator documentation, and the "not yet crash-safe" warning removed **in the change that closes M2c** | The last stage |
| P4 | **M2c's closure assessment**, then the **independent closure review** | RFC 100 requires a reviewer who did not author it. As for its design review, that is the owner |
| P5 | **RFC 151**, password policy | Small, security-relevant, independent of the console. It runs while the architect reviews a stage |
| P6 | **RFC 097 stage 1**, the inventory contract and its gate | Independent of every build above. Placed after P3, so rotation's boundary is inventoried as **finished**, not as a moving target |
| — | **The RFC 148/107 closure assessment** | The architect's own; no dispatch |

**`0.95.0` — the console follows the operator.**

| # | Work | Why here |
|---|---|---|
| Q1 | **RFC 149**, with every carried item | Its structure first: the sidebar, the page layout and the forms |
| Q2 | **RFC 101**, then **RFC 150** on the new structure | They add pages and forms. Built after RFC 149, they are built once, in their final place, not twice. This is the reason O2 went before stage 2 |

**Why M2c first.**
- **It finishes a security property** that today exists only in part.
  Since `0.93.0` the startup check and preparation-prefix recovery are
  real, but rotation itself still runs the old sequence, under the
  "not yet crash-safe" warning.
- **It leaves nothing half-built across a release** for longer than it
  has to.
- **The console has no such clock.** It is better done whole, once, in a
  release of its own.

## 3 — M2c's closure evidence, and one addition

RFC 100's acceptance criteria:
- every recovery row has an executed crash-injection test;
- no prefix leaves mixed ciphertext, a missing key, or an overstated
  audit record;
- K04 and K05 use the Class-A seam, with rollback evidence;
- the documentation is accurate;
- an independent closure review accepts the evidence.

**Proposed addition: a drill on the release binary**, run by the
architect, with no time needed from the owner. Build the `0.94.0`
candidate and rotate on a throwaway instance. Then kill the process at
three named points: after `Intent`, after K07, and after activation.
Restart each time, and record that it reaches `Complete` or `OldReady`
as the table says. The harness covers every prefix in simulation. This
shows the shipped binary does the same against a real filesystem, which
is the lesson of M4-B: the live run found what the tests could not.

## 4 — questions for the owner

1. **Split cycle P** into `0.94.0` (M2c) and `0.95.0` (the console and
   Track D), as §2 recommends? Or keep cycle P as one release?
2. **The order inside `0.94.0`**, as §2's table gives it?
3. **M2c's closure**: the owner as its independent reviewer, as for its
   design review, and the release-binary drill (§3) added to the evidence?
