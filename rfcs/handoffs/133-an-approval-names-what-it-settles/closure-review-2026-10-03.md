# RFC 133 — closure review

**Date:** 2026-10-03
**Reviewed by:** the architect, **which authored RFC 133.** This closure review is
therefore **not independent of its subject**, the same limitation recorded on RFCs
124, 128, 130 and 132, and carried under `ROADMAP.md` R1's residual. RFC 000
assigns that role to `@nabbisen` where no independent one exists.
**Status of RFC 133: `Accepted`.** This review recommends closure; it does not
record it. **Closure is not yet granted.** The `Closure approved by.` field must
stay empty until his own words exist to put in it.

## Verdict

**All four closure prerequisites are met. I recommend closing RFC 133 to `done/`.**

Each prerequisite is a checkable claim about an artifact, not a judgement, so each
is verified below by reading the artifact and by the negative test that holds it.

| Prerequisite | Verified at | Negative test |
|---|---|---|
| The template states what an `Approved by.` field should contain and why | `rfcs/README.md:374` | — (prose) |
| G11 cannot pass an `Independent design review` field whose only citation is a review of a different RFC | `scripts/check-rfc-integrity.py:636-665` (`check_review_subject`) | `test_review_subject_allowlist_entry_for_a_different_rfc_does_not_admit` |
| Every allowlist entry carries a reason, and a blank reason fails | `scripts/check-rfc-integrity.py:694-698` | `test_review_subject_allowlist_blank_reason_is_a_policy_error` |
| The docstring states that filing location is a **proxy** for subject | `scripts/check-rfc-integrity.py:44` and `:642` | — (docstring) |

`python3.14 -m pytest scripts/tests/test_rfc_integrity.py -q` → **68 passed, 26
subtests passed**. G11 passes on the working tree at `df4f25c`.

## Detail worth recording

**The blank-reason failure says why it exists.** `check-rfc-integrity.py:698`
fails with *"an exemption without a reason is a hiding place, not an allow-list"*.
An error message that states the principle rather than the rule is what stops the
next person from adding a fourth entry with `reason = "legacy"`.

**The allowlist is prevented from widening.** `test_the_allowlist_does_not_silence_condition_14`
asserts that an entry admitting a shared review does **not** suppress condition
14. That is the failure mode exemption mechanisms actually die of — they are
introduced narrowly and then quietly become general — and it is tested rather
than merely intended.

**The check is not `accepted/`-only.** `test_review_subject_checked_for_done_rfcs_too`
holds condition 9c over `done/` as well. I note this specifically because I
previously mislabelled RFCs 093/102/103 as "pre-threshold" when the real reason
they sat outside condition 9 was that *that* condition is `accepted/`-only. 9c is
not, so the same confusion cannot recur here.

**RFC 133 satisfies its own rule.** Its `Independent design review` field cites
`rfcs/handoffs/133-an-approval-names-what-it-settles/security-review-2026-10-02.md`
— under its own `133-` directory, so it passes condition 9c without an allowlist
entry. A governance RFC that would have needed an exemption from itself would be
weak evidence for the rule.

**The three allowlist entries are all substantive.** 095→094 and 096→094 cite the
one 2026-08-26 correction review that measured the three together; 103→102 cites
the shared 2026-09-16 design review. None is a placeholder, and each names the
file it relies on.

## What this review does not establish

**It is not independent.** I wrote the RFC, I wrote the gate it specifies, and I
am now reviewing its closure. Every finding above is a measurement that a
disagreeing reader can re-run from the paths and test names given, which is the
most this arrangement can offer — it is not a substitute for a second pair of
eyes, and `ROADMAP.md` R1 should keep carrying it.

**One thing I deliberately did not do:** condition 9c checks *filing location*,
and the RFC's own docstring calls that a proxy. I did not verify that each cited
review is genuinely *about* the RFC citing it — that would require reading three
reviews and judging their subject matter, which is a design-review question, not a
closure question. The proxy's limits are stated in the artifact, which is what
prerequisite 4 requires; closing the RFC does not claim the proxy is tight.

## Recommendation

Close RFC 133 to `done/`, with `Closure reviewed on. 2026-10-03`. The
`Closure approved by.` field then records his own words and the non-independence
stated above — **not before they exist.**
