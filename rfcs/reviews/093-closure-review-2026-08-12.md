# RFC 093 — independent closure review

**Date.** 2026-08-12 (Asia/Tokyo)  
**Reviewer.** `codex-independent-architecture-security-reviewer` (OpenAI Codex)  
**Request.** [Closure review request](./093-closure-review-request-2026-08-03.md)  
**Baseline.** `1d58da2491275409e9012ae0482af62bb86934ba`  
**Governing RFC.** [RFC 093](../done/093-build-toolchain-release-gates.md)

## Review result

**Outcome: Corrections Required.** RFC 093 must remain `Accepted`; do not move it to `done/` or add closure-approval metadata.

The hosted M1a and M1b runs substantiate the required build/gate work. Closure is nevertheless blocked because the retained legacy audit script is still represented as a completeness/structural assurance in a normative audit document. That is the specific claim RFC 093 requires this independent review to reject.

## Hosted-evidence verification

I independently queried the hosted CI run metadata and job outcomes rather than crediting the prior architect's closure records as evidence.

| Evidence | Independently observed result |
|---|---|
| M1a closure run `30546346612` | `push`, commit `474d0f2d70d8c9fefca6aae6aaf4fb68b1a84809`, conclusion `success`, 18/18 jobs successful. The observed job set includes G01–G09a/b, G12, A3.2, and A3.4. |
| M1b closure run `30754447964` | `push`, commit `1d58da2491275409e9012ae0482af62bb86934ba`, conclusion `success`, 17/17 jobs successful. The observed job set includes G10a, G10b, G11, G12, and A3.4. |
| C0 run `30555102811` | `push`, commit `b0cca2f9f733f5fcfcc4a658a9e6da095b8e66fa`, conclusion `success`, 19/19 jobs successful. |
| C1 run `30688197346` | `push`, commit `698cc66347aa765bb8b58417258741baac9527fa`, conclusion `success`, 20/20 jobs successful. |
| C2/C3/C4 run `30735008212` | `push`, commit `bfbb2e42e6acd735e8f5d872a00f261b64589c70`, conclusion `success`, 21/21 jobs successful. |
| C2.1 run `30744684897` | `push`, commit `afc999e75c20ede290bb01ea6c3b43c803575b3c`, conclusion `success`, 21/21 jobs successful. |

The CI definition also makes the per-lane evidence shape an executable condition: the dispatcher records and compares `event_commit` and `checked_out_commit`, requires a clean tree, records the command, and reports its exit status ([`scripts/ci-gate.sh`](../../scripts/ci-gate.sh) lines 79–113). G12 and A3.4 carry equivalent explicit checkout checks ([`ci.yml`](../../.github/workflows/ci.yml) lines 383–424 and 437–468).

The C5 diff was also inspected. It removes only the four legacy inline UI jobs and retains `ui-invariants-v1`; the retained job documents its four blocking checks and negative self-tests ([`ci.yml`](../../.github/workflows/ci.yml) lines 365–424). The absence of negative fixtures for G12's two advisory reports does not invalidate those blocking checks.

## Blocking finding

### B-093-1 — the legacy literal check is represented as structural audit assurance

RFC 093 is explicit: the legacy script compares event-name strings and cannot prove emission, typing, or shared mutation/audit transactions ([RFC 093](../done/093-build-toolchain-release-gates.md) lines 47–49). Its required wording is “Diagnostic only,” and says string parity proves neither emission completeness nor mutation/audit atomicity (lines 261–269). The closure handoff makes independent confirmation of that limitation mandatory ([verification handoff](../handoffs/093-build-toolchain-release-gates/verification.md) lines 74–79).

The current normative [audit coverage matrix](../../docs/src/reference/audit-coverage-matrix.md) contradicts that boundary in two connected ways:

- it says every privileged state mutation has a row and that `check-audit-matrix.sh` keeps the document and code in sync (lines 3–6); and
- it says that adding a privileged operation without updating the matrix is a CI failure (lines 140–148).

The script cannot establish either proposition. It extracts backtick-quoted event names from Markdown and audit-namespaced string literals from Rust, then compares the two sets ([`check-audit-matrix.sh`](../../scripts/check-audit-matrix.sh) lines 27–69). A mutation can omit an event literal, use a misleading literal, or append outside its transaction and still pass. The known `user.reset_mfa` / `mfa.admin_reset` mismatch further demonstrates that a name-based relation is not a structural proof.

**Required correction.** Before a new closure review, amend the matrix and every derived active assurance claim so the script is described only as vocabulary/string parity. In particular, replace the claim that a new privileged operation is caught with the narrower claim that a newly introduced matching event-name literal without a matrix row is caught. Place the required diagnostic-only limitation next to the script description and state that RFC 094 owns the structural completeness and atomicity authority. Do not represent the matrix's desired policy as something the legacy script has mechanically established.

## Known items assessed

| Item | Assessment | Closure effect |
|---|---|---|
| README's 26 external links are outside G10b's local-target check. | A documented coverage boundary, not an assertion that external reachability was tested. The link check's output must be described as local-link integrity only. | Non-blocking; track external-link policy under RFC 098/M5 if desired. |
| Eight mdBook links resolve in the repository but not in rendered book output. | A real documentation defect, but `mdbook build` does not claim rendered outbound-link validation. | Non-blocking for RFC 093's stated G10a contract; repair under the documentation-authority work. |
| The RFC README template omits `Accountable owner and approver`, although G11 requires it for prospective RFCs. | A live usability/process defect: the documented template can produce an RFC G11 rejects. The actual checker correctly enforces the field ([`check-rfc-integrity.py`](../../scripts/check-rfc-integrity.py) lines 75–83 and 399–408). | Non-blocking for this closure, but should be repaired as the small standalone M1b-owned documentation correction identified in the recorded analysis. |
| RFC 000 and RFC 018 share a lifecycle-policy title without a supersession marker. | The recorded analysis shows that they contain distinct, live normative material, so neither can safely be archived as a duplicate. The ambiguity is a documentation-authority issue, not a false G11 result. | Non-blocking; require an explicit authority/consolidation decision under RFC 098/M5. |
| G12's advisory reports have no negative fixtures. | Deliberate and accurately labeled: the advisory reports do not control the G12 result, while the four blocking invariants have negative self-tests. | Non-blocking. |

## Assessment of the prior reviewer’s evidence claims

The M1b closure record acknowledges five defects, including two reported verifications that were not performed (`#backups` and C5 annotations). That is an evidence-quality failure, not a harmless editorial error. I therefore did not use the ignored closure records as proof of any requirement.

For the milestone-level result, the pattern is contained sufficiently to evaluate the hosted CI outcome: the run identities, commits, job names, and successful conclusions above were independently observed from GitHub Actions, and the workflow contains the relevant SHA/clean-tree/exit-status assertions. It is **not** contained sufficiently to credit uncorroborated ancillary statements from those records, such as log-line counts or annotation interpretations. Any renewed closure request should cite durable, directly inspectable hosted evidence for each such claim rather than relying on `.git-exclude/` prose.

This review does not find the prior reviewer’s defects to invalidate the observed hosted run outcomes. It does require the project to stop treating unverified reviewer narration as evidence.

## Required path to approval

1. Correct B-093-1 in the audit matrix and all active assurance documentation; preserve the diagnostic-only wording required by RFC 093.
2. Run the applicable M1b/documentation gates on one clean commit after that correction and retain the hosted run identity and job evidence.
3. Request a focused closure re-review. If approved, then—and only then—record `Closure reviewed on`, `Closure approved by`, and a tracked, resolving `Closure evidence` link before moving RFC 093 to `done/`.

---

`rfcs/reviews/093-closure-review-2026-08-12.md`
