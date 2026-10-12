# The five open decisions, reviewed against the fundamental philosophy — two of my recommendations were wrong

**Written.** 2026-10-12 JST, by the architect.
**Status. Recommendations, not decisions.** All five remain open.
**Why this document exists.** Asked to review my own five recommendations
against the philosophy they are supposed to serve:

> **"finally clean, safe and secure, and robust and sophisticated design"** —
> and **"APIs and UI/UX for users not to be confused or misunderstand are also
> very important"**.

**Two of the five do not survive it.** Items 1 and 4 are withdrawn and
replaced; items 2, 3 and 5 stand, with item 3 promoted from "later" to
"before the release", which is the change that makes item 4's replacement
necessary.

---

## Item 4 — withdrawn. I recommended shipping the confusion

**What I recommended:** cut `0.80.0` after stage 6c, now.

**What `0.80.0` would then put in front of a user.** `link_only` is the
**default** `provision_mode`. Under it, an upstream identity with no existing
link is refused with `fed_error=link_required` — and **nothing renders
`fed_error`**. So the user is redirected to a login page that says nothing at
all, with an unexplained query parameter in the address bar, and there is no
self-service linking anywhere in the product to send them to. The operator's
only recourse is writing a `federation_link` row into the database by hand,
which is what `docs/src/guides/operators.md` now says in as many words.

**That is the second half of the philosophy, failed directly.** A user who
cannot sign in and is told nothing is the definition of confused, and an
operator who configures the default mode and discovers the dead end only in
production has been misled by a release.

**And I recommended it while separately recommending the fix for it later.**
Item 3 — the surfacing stage — *is* the remedy for "nothing renders
`fed_error`", and I placed it after stage 6d, which is after the release I was
proposing. Items 3 and 4 contradicted each other and I did not notice, because
I was weighing the release against *code* readiness (the growth leak closed,
the gates green, the fixes landed) and treating the user-facing half as
schedulable afterwards. **That is the same mistake as calling the `exp`
divergence "non-blocking": the UI/UX half is not the part that waits.**

**Replacement recommendation.** The release precondition is not "stage 6c
landed". It is: **a user who reaches the federation path either signs in or is
told, in the product, what to do.** Concretely, `0.80.0` ships after:

1. **Item 3's surfacing stage** — the bounded map from the `fed_error` family
   onto a small set of user-visible messages, including an honest one for
   `link_required`.
2. **The operator guide saying the thing plainly**, not euphemistically: under
   `link_only` this release has no self-service linking, so a first link is
   created either by switching that provider to `provision_on_first_login` or
   by an administrator inserting the row — and which of those an operator
   should choose.
3. **Stage 6c's own landing**, which is done.

Stages 6d and 8–11 (F01 and the MFA continuation) are **not** release
preconditions: they change no behaviour an operator sees that is not already
either working or explained. Stage 12's removal of the legacy path is not one
either.

**What this costs.** The release moves from "now" to "after the surfacing
stage" — one stage, which needs your authorization (item 3). It does not move
to "after all of 096-B1".

**The `target/` consequence moves with it.** The controlled-removal trigger is
the release boundary, so it fires then rather than now. The current measurement,
taken for this document: **`target` 400 GB, `/home` 85% used, 309 GB free.** If
you would rather not wait for the trigger, say so and I will ask at a moment of
your choosing instead — the procedure is unchanged either way (measure, ask,
remove only `target/`, re-measure, report the measured figure).

---

## Item 1 — withdrawn. I used "no gate checks it" to justify parking a contradiction

**What I recommended:** implement F01 as Class A, record the divergence from
RFC 094's prose in RFC 096's handoff and the manifest's `rationale`, and amend
RFC 094 after 096-B1 closes.

**Why that fails the first half.** The argument I leaned on was that *no gate
ties the manifest's class to RFC 094's prose, so the build is fine either way*.
That is a statement about what is **checked**, not about what is **true**. It
recommends knowingly shipping a code/record disagreement and filing the truth
in a handoff document — a documented contradiction, parked, with a gate's
silence as the permission slip. "Finally clean" does not admit that, and I have
been corrected on exactly this shape once already.

**And the cost I used to justify it was overstated — measured, not assumed.** I
wrote that amending RFC 094 *"could halt the project it was meant to tidy"* and
*"halt every lane the command registry gates"*. What a return to `proposed/`
actually blocks:

| Lane | Blocked? |
|---|---|
| RFC 094 **M2b** (its own remaining implementation) | **Yes** — and M2b is unstarted, 43 commands unconverted, not next on any schedule |
| RFC 095 (M3, Accepted, implementation open) | **No** — its prerequisite is *"RFC 094 M2a Implemented"*, a milestone state that is met and that a file move does not un-meet |
| 096-B1's remaining stages | **No** by that mechanism — they implement RFC 096, and M2a's foundation is delivered code |
| `check-rfc-integrity.py` | **No** — it checks folder/status agreement and acceptance metadata; it does not cross-check one RFC's prose claim about another's status |

So "halts everything" was wrong. **But measuring also found a real cost I had
not named, and it is the one that matters:** RFC 096's own *Design
prerequisites* line reads *"amended RFC 094 Accepted"*. While RFC 094 sits in
`proposed/`, that line is false — so continuing to implement 096-B1 during the
window means building on a prerequisite the repository itself contradicts. No
gate catches that either, which is precisely why it is not a defence.

**Replacement recommendation — a third option neither of us had on the table:
sequence the amendment into a window where nothing is in flight, and build F01
after it.**

1. **Do not dispatch stage 6d now.** F01's design is settled; what is not
   settled is the record it would be built against, and building first is what
   creates the contradiction.
2. **Move 6d to sit after stage 11 and before stage 12.** F01 must precede
   stage 12 (the legacy path's removal takes the existing-link no-MFA path with
   it), but it need not precede the MFA continuation. So stages 8–11 run first,
   unchanged.
3. **Amend RFC 094 in the gap between stage 11 landing and stage 12 being
   dispatched** — a deliberate pause with nothing dispatched, so no stage is
   ever implemented against a false prerequisite. The amendment carries all
   three recorded divergences at once (M2a's exit-criteria consolidation's
   successor, `ReadConn`, and `emit_must_attempt`'s milestone) plus F01's
   class. It needs re-acceptance, not re-design.
4. **Then stage 6d builds F01** against a record that says what the code does.

**What this costs.** One pause in the schedule, at a point chosen rather than
forced, and F01 lands later than it otherwise would. **What it buys:** no
interval in which the code and an Accepted RFC disagree, and no handoff
document carrying a truth the RFCs deny.

**If you would rather not reopen RFC 094 at all**, the honest alternative is
not my withdrawn recommendation — it is to build F01 **as RFC 094 records it**,
Class P with a post-commit Class-B event, and accept that
`auth.federation.signin.success` loses the atomic audit L04 gives it today.
I do not recommend that, because it trades a real security property for a
process convenience, but it is the option that keeps every record true without
an amendment, and you should have it stated rather than hidden.

---

## Item 2 — stands, and the cleanliness argument is stronger than I put it

The `(A required)` marking on `auth.federation.takeover_blocked`: my
recommendation to amend it for that one row is unchanged. What I under-stated is
that **leaving it is also a contradiction left in a contract file**: the marking
records a requirement that, for this row, cannot be meaningfully met — the
collision path mutates nothing, so there is nothing for an atomic commit to be
atomic with, and A6 argues against the attempt. A contract that requires an
impossibility is not clean either. The eleven other rows are untouched and M2b's
scope is unchanged.

## Item 3 — stands, and is promoted

Unchanged in substance: a bounded map from the `fed_error` family onto a small
set of user-visible messages, never a rendering of the internal value, with
every collision-adjacent value collapsing into the same message. **What
changes is its position: it is now a release precondition** (item 4), not a
post-release tidy. It is the only one of the five that the second half of the
philosophy makes urgent rather than merely correct.

## Item 5 — stands

096-A's acceptance remains reserved to you under `:23`. Nothing in this review
touches it; the completion assessment is unchanged.

---

## What changed in my own practice

Both withdrawn items failed the same way, and it is worth naming once rather
than discovering a third time:

- **Item 4** weighed a release on code readiness and left the user-facing half
  to a later stage. The philosophy has two halves and the second one is not the
  schedulable one.
- **Item 1** reached for *"no gate checks it"* as a reason to tolerate a
  divergence. A gate's silence is not a cleanliness argument, and when I used it
  as one I also failed to measure the cost of doing it properly — which turned
  out to be smaller than I claimed in one direction and realer than I knew in
  another.

The common root is that I was optimising for *unblocking the next dispatch*.
That is the right instinct for scheduling and the wrong one for deciding what
"finished" means.
