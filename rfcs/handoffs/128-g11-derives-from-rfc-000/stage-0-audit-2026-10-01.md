# RFC 128 stage 0 — audit of the current status

**By.** High-capability model, architect. **This audit is the architect's own
work, not the dev team's**: it judges whether design boundaries were crossed and
which governance statements are load-bearing, both of which are design and
direction. Dispatching it would repeat the error it exists to measure.
**Date.** 2026-10-01. **Criterion**, `@nabbisen`'s: *does the statement
presuppose a skilled architect external to this team?* *"No such a person out of
our team."*
**Status.** **Part 1 complete** (the load-bearing statements). **Part 2 scoped,
not done** (the fifteen packages). Nothing under `rfcs/` was changed.

---

## Part 1 — the load-bearing statements. The programme cannot reach M7.

This is the finding. It is not a labelling problem.

### Milestone exit gates

| Milestone | Exit gate requires an independent reviewer? |
|---|---|
| M0, M1a, M1b, **M2a**, **M4-A** | **No** — satisfiable |
| **prep** — federation module split | **Yes** — *"independently reviewed by someone other than its implementer, **before 096-A or any RFC 094 federation work begins**"* |
| **M2b** | **Yes** — *"independent adversarial closure review accepts the evidence"* |
| **M2c** | **Yes** — *"independent adversarial closure review accepts the crash-injection evidence"* |
| **M3** | **Yes** — *"independent review"* |
| **M4-B** | **Yes** — *"independent security review pass"* |
| **M6** | **Yes** — *"independent review approves soak entry only"* |
| **M7** | **Yes** — *"owner and independent reviewer accept the evidence"* |

**Every milestone from M2b onward has an unsatisfiable exit gate**, including M6
(soak entry) and M7, which is the programme's readiness goal. And the `prep` item
blocks **096-A and all RFC 094 federation work** on the same unsatisfiable
condition.

### RFC closure prerequisites

| RFC | Closure prerequisite |
|---|---|
| **094** | **M2a: no such requirement** — which is why M2a is closeable. **M2b:** *"independent adversarial closure review accepts durable evidence"* |
| **095** | *"independent closure review accepts evidence"* |
| **096** | **096-A:** *"pass independent review"*. **096-B1** and **096-B2:** *"pass independent closure review"* |

### What RFC 000 actually requires, for comparison

RFC 000 requires, at shipment, `Closure reviewed on`, `Closure approved by`, a
durable `Closure evidence` reference, and that **the implementer is not the sole
approver**. **All four are satisfiable** — `@nabbisen` is not the implementer.

**The same pattern as G11 condition 9:** RFC 000's requirement is satisfiable;
the July-era RFCs' *additions* to it are not. The word doing the damage is
"independent", used without a definition RFC 000 supplies and without a person
who could satisfy it.

### Consequence, stated plainly

The remediation programme has been unable to complete since July 2026, by its own
written terms, and nothing detected it — because no milestone after M2a has been
attempted. M1a and M1b closed; M2a is in progress. **The first time the programme
reaches M2b, it stops.**

`@nabbisen` has said he may engage an external architect as a temporary exception
if necessary. **This is where "if necessary" is answered: seven exit gates and
five closure prerequisites, or those statements are re-scoped to what this team
can do.** That is his decision and this audit does not make it.

---

## Part 2 — the fifteen packages. Scoped, not done.

The fifteen where the dev team reviewed the architect's design: RFCs **094, 095,
096, 102, 103, 110, 112, 115, 116, 117, 118, 121, 122, 123, 126**. Eleven
dispatched by the current architect in the two weeks to 2026-10-01; four predate
him.

**Method, fixed now so the result is not shaped by the reading order:** each
package's content classified as **measurement**, **enumeration**, **feasibility**,
or **a judgement about whether the design was right** — the last being the only
category that crossed the line. One row per package, with the `file:line` of each
judgement found.

**Three are already known, all adopted by the architect, which is how they became
decisions:**

| RFC | The judgement | What the architect did |
|---|---|---|
| 121 | D3 settled as `tracing` primary with a best-effort audit row | Adopted; later corrected the handoff wording that said "the review resolved it" |
| 123 | Two rate-limit buckets rather than one | Adopted; the argument dissolved a tension the architect had posed |
| 126 | A caller meeting the bound **queues** rather than being refused | Adopted; **later corrected the RFC to record the decision as the architect's**, having asked the review to settle it |

**Not done, and not guessed at.** The remaining twelve need reading, not
inference. The architect will not estimate what they contain.

---

## What this audit does not do

It changes nothing, corrects nothing, and retracts no finding. RFC 128's D1–D5
wait on it, as its handoff requires, **because Part 1 changes what D2's audit of
G11 is for**: condition 9 is one instance of a pattern that also reaches seven
milestone exit gates and five closure prerequisites, and fixing the gate alone
would leave the programme still unable to finish.
