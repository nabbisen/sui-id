# RFC 121 — independent design review

**RFC.** [RFC 121 — A verification failure is not a pass](../../done/121-a-verification-failure-is-not-a-pass.md). **Proposed.**
**Handoff reviewed against.** [`rfcs/handoffs/121-a-verification-failure-is-not-a-pass/design-review-request.md`](../../handoffs/121-a-verification-failure-is-not-a-pass/design-review-request.md), [`README.md`](../../handoffs/121-a-verification-failure-is-not-a-pass/README.md).
**Baseline.** `5dba3df` (`= 4ebf0f7` plus doc-only commits; no source file differs). Nothing changed by this review.
**Reviewer.** Mid-capability model, implementer role. I did not author RFC 121; I did confirm its founding defect in the RFC 120 triage.
**Scope.** Read-only, as instructed. **One exception, disclosed here in full:** to answer items 2 and 5 honestly I needed to know, not guess, whether `verify_chain_tail`'s "intact" result means what its own doc comment claims. I built two throwaway unit tests inside a disposable `git worktree` (`git worktree add --detach`, never inside this checkout), ran them, read the result, and removed the worktree (`git worktree remove --force` + `prune`) before writing anything else. `git status` in this checkout is clean before and after — verified below. No file in this repository was changed. This is the same device used for the mutation-testing evidence in the RFC 112 and RFC 120 packages; I am naming it explicitly here because what it found changes the review's whole shape.

```
$ git status --short   (before and after; identical, empty)
$ git worktree list
/home/nabbisen/Desktop/sui-id/sui-id-git  5dba3df [main]
```

---

## 0. The finding that reframes this review

**`verify_chain_tail` does not verify a chain.** It verifies, independently for each row, that the row's own `hash` column is the correct hash of that row's own `prev_hash` column plus its own content. It never compares one row's `hash` against the next row's `prev_hash`. So an attacker with the exact access the module's own threat model names — raw SQL, no application code — can change any one row's content and restore internal consistency with a **single `UPDATE`**, using that row's own unchanged `prev_hash`, and `verify_chain_tail` reports the tail **intact**. No other row needs to change. This directly contradicts three places the codebase states the opposite:

- `crates/sui-id-store/src/repos/audit.rs:4-10` (module doc): *"To rewrite or delete row N you must recompute every subsequent row's hash — something an attacker with raw SQL access can do, but they cannot do it without leaving any trace…"*
- `crates/sui-id-store/src/migrations/0009_audit_hash_chain.sql:5-7`: *"An attacker who compromises the SQLite file and tries to delete or rewrite a row breaks the chain at the next row, which is detectable…"*
- `crates/sui-id-store/src/repos/audit.rs:361-380` (the existing test `tampering_with_a_row_makes_chain_verification_fail`), whose own comment reasons about a chain-linkage check that the code does not perform. That test tampers a row's `action` **without** updating its `hash` column, so it is caught by the *single-row formula* check — the only check that exists. It is not a test of chain linkage; nothing in the suite is.

**Proved, not argued.** In the disposable worktree: `crates/sui-id-store/src/repos/audit.rs:246` (`verify_chain_tail`), three rows appended, row 2's `action` changed to `"tampered"` and its `hash` column recomputed via `compute_hash` (`audit.rs:90`) using row 2's own **unchanged** `prev_hash` — every other field (`at`, `actor`, `target`, `result`, `note`) read back from the row verbatim so the hash would be wrong for no reason but the intended one. Result: `ChainVerifyReport { checked: 3, broken_at_seq: None, legacy_unhashed: 0 }`. **The tamper is undetected.** No triggers or constraints on `audit_log` compensate (`grep TRIGGER` across every migration: none).

**Why this outranks everything else in the review request.** RFC 121 assumes the three outcomes are *verified intact* / *verified and broken* / *could not verify*, and its whole job is making the third one honest. But "verified intact" is not honest either, against the exact attacker the module names (`audit.rs:14-16`: *"Local detection is enough for 'DB-only access' attackers, which is by far the more common attack model for a self-hosted IdP"*). Fixing the display layer, as scoped, would ship a chain-integrity page that says "intact" in green, truthfully reporting the outcome of a check that does not check the thing it is named for. Item 6 of the review request anticipated something in this shape — *"this fixes a display bug on a control that cannot do what its users think"* — and invited exactly this finding. It is worse than that framing: the tail-window and external-anchor limits (§2 below) are honest limits of a real control; this is the control not implementing its own stated property.

---

## 1. Confirming the defect, and every caller of `verify_chain_tail`

**Confirmed at the baseline, exactly as stated.**

- `crates/sui-id/src/http/handlers/admin/audit.rs:37-44`: `audit::verify_chain_tail(&app.db, 500).await.unwrap_or(ChainVerifyReport { checked: 0, broken_at_seq: None, legacy_unhashed: 0 })`, then `chain_ok = chain.broken_at_seq.is_none()` (`:44`). An `Err` and a genuine "verified, nothing wrong" result produce the identical `ChainVerifyReport` and the identical green banner. The substituted report is not logged anywhere.
- `crates/sui-id/src/http/handlers/settings.rs:290-297`: the same call, `.map_err(|e| HttpError::html(CoreError::from(e)))?`. `CoreError::Store(_)` renders as `"An internal error occurred."` (`crates/sui-id/src/http/errors.rs:319-323`) with a 500 — an error page, as the RFC says. **Worth adding:** this `?` is on the whole `logs_get` handler (`settings.rs:252`), so a chain-verify error today takes down the *entire* logs tab — the 24-hour login/lockout/password-change counters too, not just the chain status. That is a stricter failure mode than the RFC describes, in the opposite direction from the audit page's silent pass.

**Every caller, confirmed exhaustively** (`grep -rn "verify_chain_tail" crates --include='*.rs'`, non-test): exactly the RFC's three — `admin/audit.rs:37`, `settings.rs:291`, `runtime/startup.rs:173` — plus the function's own definition and its five existing unit tests in `audit.rs` itself. **No fourth caller exists.** The RFC's assumption holds.

**Startup, read precisely** (`runtime/startup.rs:173-192`):

```
match verify_chain_tail(&db, AUDIT_VERIFY_TAIL).await {
    Ok(report) if report.broken_at_seq.is_some() => tracing::error!(...)   // :176-181
    Ok(report)                                    => tracing::info!(...)   // :183-188
    Err(e)                                        => tracing::warn!(...)  // :191
}
```

`AUDIT_VERIFY_TAIL = 5_000` (`:33`). **A fact the RFC does not state and should:** the three outcomes are not just "different sentences by accident" (RFC summary) — they are **different `tracing` levels**: `error!` for broken, `warn!` for could-not-verify, `info!` for intact. An operator who filters logs at `error` (a common baseline) sees a broken chain but never sees a chain that could not be checked at all. That is a second, independent way this defect is worse than a wording mismatch, and D2/D4 should name it.

---

## 2. Is "three outcomes" the right shape? (item 2)

**No — not because a fourth outcome is missing, but because outcome 1 does not mean what D1 implies it means, for two separable reasons, and the RFC currently addresses neither.**

**(a) Window coverage (the question as literally asked).** `verify_chain_tail(db, limit)` walks the most recent `limit` rows only (`ORDER BY seq DESC LIMIT ?1`, `audit.rs:249-251`). "Verified intact" truthfully covers only that window: the startup call sees the newest 5,000 rows, the audit page 500, the settings page 100 — three different windows answering "is the chain intact" with three different scopes, none of which is "the whole log since genesis." **State it plainly, as the request asked:** *verified intact* means *the most recent `limit` rows are internally consistent*; it says nothing about row `seq` 1 through `total - limit`. On an instance old enough to exceed any of these limits, "intact" has quietly stopped meaning "the whole history is intact" and nothing on any of the three surfaces says so.

**(b) Linkage (§0, found during this review, not asked for by name but squarely what item 6 invited).** Even *within* the checked window, "intact" does not mean "each row's `prev_hash` is the previous row's actual `hash`" — that comparison is never made. So (a) and (b) compound: even a determined attacker confined to a recent window, using only the row-formula tamper in §0, produces "verified intact" with no window-boundary evidence of anything wrong, because the property being checked was never chain linkage in the first place.

**Recommendation for D1:** state outcome 1 as *"the most recent `limit` rows each pass a local hash check"* — accurately — until (b) is fixed, at which point it can honestly become *"the most recent `limit` rows form a linked, internally consistent chain."* Do not let the wording imply more than either version delivers. A **fourth outcome is not needed**; what is needed is that outcome 1 stop overclaiming. I would not gate D1's acceptance on fixing (b) inside RFC 121 — the RFC's stated scope is display-only — but the RFC must not ship language implying (b) already holds, and should name it as follow-on work rather than stay silent.

---

## 3. D3 — does a failure to verify belong in the audit log itself? (item 3)

**For:** the audit log is the durable, queryable, exported record operators already use (CSV export exists, `admin/audit.rs`). A verification failure is itself a security-relevant event — RFC 102's own precedent (cited by the RFC) is that an unauditable event is not silently dropped. If the failure is transient (a lock timeout, a momentary I/O error), a row lets an operator later ask "how often has this happened" from the one place they already look.

**Against:** `append`/`append_within_tx` (`audit.rs:97`, `:155`) hit the **same connection** that just failed to be read for verification. If the cause is store-wide (the file is locked, corrupt, or the disk is gone), the write is likely to fail too, either silently swallowed (existing best-effort call sites use `let _ = audit::append(...)`) or itself erroring in a way that must not cascade into the verification failure path. Worse: **if the eventual cause of "could not verify" is exactly the tamper this whole system exists to catch** (§0's kind of row, or a genuinely corrupted `audit_log` table), then trusting the audit log to record its own failure is circular — the record of "I couldn't read myself" lives in the thing that might be the problem.

**My recommendation:** log it at `error` level via `tracing` (not `warn`, correcting §1's finding), unconditionally — that channel does not depend on the store being healthy. **Attempt** an audit-log row as a best-effort, secondary signal (`let _ = audit::append(...)`, matching the codebase's existing pattern for failures that must not cascade), explicitly not on the critical path and not required to succeed. Do not make the tracing line's success depend on the audit row's success, and do not treat the audit row as the primary record — the RFC's own doubt about it ("of doubtful value when the cause is the audit store itself") is correct, and the resolution is that the row is a nice-to-have, the trace line is the requirement.

---

## 4. D4 — startup, and is this the same kind of thing as RFC 112's refusal? (item 4)

**Today, for each outcome (confirmed in §1): none is fatal.** The process starts regardless. The code says why (`startup.rs:167-171`): *"We don't refuse to start on detection — that would let an attacker DoS the IdP by corrupting one row."*

**Is a verification failure the same kind of thing as RFC 112's schema refusal? No, and the RFC should say so explicitly rather than let the analogy stand unexamined, which is exactly what the review request warned is likely to happen.**

RFC 112's refusal is a **structural precondition**: can this binary's migration runner safely operate on this file at all. It is deterministic, checked read-only before any write, and every legitimate database eventually satisfies it (run the right binary, or restore a backup). Refusing to start is the *safe* action because the alternative (writing to a schema you might misunderstand) is the dangerous one.

Chain verification is a **best-effort tamper monitor**, not a precondition for safe operation — the store operates correctly whether or not the chain has ever been checked. Making "could not verify" fatal would mean: an ordinary transient I/O error, a `busy` timeout under load, or (per §0) even the malformed-row shape used in my D5 test, takes the whole IdP offline. That is a worse outcome than the status quo for the overwhelmingly common cause of "could not verify" (a filesystem hiccup), and — because of §0 — it does **not** reliably buy protection against the attacker it is meant to catch: a competent attacker following §0's method produces "verified intact," not "could not verify," so making the *third* outcome fatal hardens the system against the *least* dangerous of the three failure shapes while leaving the actually-dangerous one (a false "intact") completely unaddressed. The code's own reasoning about the *broken* case — refusing there would let an attacker corrupt one row to DoS the server — applies with at least equal force to the *could-not-verify* case, and the RFC should say this in D4 rather than leave the question open the way the handoff currently does.

**Recommendation: not fatal**, for both outcomes 2 and 3, unchanged from today — but D4 should say this as a **reasoned decision**, citing both the DoS argument the code already makes and §0's finding that a fatal-on-failure policy would not meaningfully improve security against the attacker being defended against. What *should* change per D2/D3: the level (§1, `error!` not `warn!` for outcome 3) and, per D1/D2, that operators reading the startup log and the two web surfaces see the same words for the same outcome.

---

## 5. D5 — the test, injected without a test-only branch (item 5)

**Confirmed possible, two ways, neither touching production code.** Both verified in the disposable worktree.

1. **A malformed `actor` value via ordinary SQL**, on a database the test already owns: `UPDATE audit_log SET actor = 'not-a-uuid' WHERE seq = 1`. `verify_chain_tail`'s row-mapping closure (`audit.rs:254-276`) parses `actor` as a UUID (`:262`) and propagates a `FromSqlConversionFailure` through `collect::<Result<Vec<_>,_>>()?` (`:278`) and the outer `.await?` (`:281`), which is a genuine `StoreResult::Err` — the same error type and path any real corruption would take. Proved: `verify_chain_tail(...)` returned `Err(Db(FromSqlConversionFailure(2, Text, Error(ParseChar { character: 'n', index: 0 }))))`.
2. A second, equally direct option not tested but evident from the code: close or corrupt the underlying connection, or drop the `audit_log` table via `db.with_conn`, before calling — `with_conn`'s own error path (`db.rs`) surfaces as the same `StoreResult::Err`.

**Recommendation:** use (1). It is the smallest, most realistic injection — a single malformed byte is a far more faithful stand-in for "real-world corruption" than dropping a table — and it requires **zero** production-code changes: no `#[cfg(test)]` branch, no injectable-failure trait, no seam. The e2e version of this test (through the real HTTP handlers, matching how the RFC 120 package tested through the router) is equally reachable: `state.db.with_conn(...)` is already used this way by other e2e helpers (`tests/e2e/common.rs`), so a test can corrupt one row on the app's own `AppState.db` and then hit `/admin/audit` and `/admin/settings/logs` in the same test, asserting both surfaces show the third outcome and agree with each other — which is D2's actual requirement, not just D5's.

**One more thing D5 should ask for, found while answering it:** a test in the same shape proves §0 — append three rows, corrupt one via SQL with a **valid** recomputed hash rather than a malformed field, and assert `broken_at_seq` is **not** `None`. On the current code that assertion fails. I recommend RFC 121's own D5 evidence include this test, marked `#[ignore]` with a comment pointing at the follow-on RFC from §0, rather than silently absent — so the gap is visible in the test suite the moment RFC 121 lands, not discovered a second time later.

---

## 6. Anything else (item 6)

**Is this worth doing before an external anchor?** `ROADMAP.md` §S2's framing (tamper-evident only within its trust boundary) is a real, separate limit from §0 — it says "even a correctly-implemented chain only defends against an attacker outside the application's own trust boundary," which is a sound and already-acknowledged limit (the module's own doc says the same, `audit.rs:12-16`). **That is not a reason to defer RFC 121.** RFC 121's job — stop the display layer from lying about what the check returned — is worth doing regardless of what the check *can* detect, because a UI that turns "I don't know" into "no problem" is wrong on its own terms even if the underlying check were perfect. Doing RFC 121 first, then following with a fix for §0, then eventually an external anchor, is the right order: fix the lie, then fix the check, then widen what the check can prove.

**What §0 changes about that order:** RFC 121 should still ship, but its closure language should not read as "the chain now honestly reports whether it is intact" — after RFC 121, it honestly reports whether the **existing, incomplete** check found something wrong. That is a narrower and more honest claim, and I'd ask the RFC to use it rather than let readers assume RFC 121 makes "intact" trustworthy.

**Smaller items, not blocking:**
- `checked` includes rows across every action type. Neither web page states the window size to the operator (`500` / `100` / not shown at all on the audit page beyond the banner); once D1's wording is fixed (§2), the number itself should probably be shown too, so "intact" carries its own scope on the page rather than only in this review.
- The settings page's `?` failure mode (§1) losing the whole logs tab, not just the chain status, is worth a one-line fix note even though it is not this RFC's stated touch point — `settings.rs` is already on the touches list.

---

## Findings, ranked

**Blocker**

1. **`verify_chain_tail` never checks that one row's `prev_hash` equals the previous row's `hash` — it checks each row's own hash formula in isolation.** A single `UPDATE` (new content, recomputed hash from the row's own unchanged `prev_hash`) is undetected: `checked: 3, broken_at_seq: None` in a reproduced, evidence-backed test (§0). This contradicts the module doc (`audit.rs:4-10`), the migration comment (`0009_audit_hash_chain.sql:5-7`), and the stated threat model (`audit.rs:14-16`) the whole control exists to satisfy. **Not in RFC 121's stated scope, but RFC 121 cannot honestly claim to fix "the chain lies about its own state" while this stands** — closure language should be scoped to match (§6), and this needs its own RFC, urgently, given the project's practice on this line of RFCs (120→121–124) of spinning out confirmed findings rather than folding them in.

**High**

2. **The three outcomes are reported at three different `tracing` levels** (`error!` / `info!` / `warn!`, `startup.rs:176,183,191`), not just three different sentences. An operator filtering at `error` never sees "could not verify." D2/D4 should say the levels must match in severity ordering, not only in wording.
3. **"Verified intact" does not state its window.** Three different limits (5,000 / 500 / 100) answer the same question with silently different scope, and none of the three surfaces says so (§2a). Not a defect RFC 121 introduces, but one it should close while it is already touching all three call sites.
4. **The settings logs page fails its entire tab, not just the chain widget, on a verify error** (`settings.rs:252-297`, one `?` on the whole handler). Confirmed behavior, not previously stated this precisely in the RFC or its handoff.

**Medium**

5. D3's audit-log-of-a-failure question has a real circularity risk (§3) the RFC should resolve explicitly rather than leave as an open question for the implementer to guess at.
6. D5's evidence should include the §0 mutation (a real linkage break) as a currently-`#[ignore]`d, visibly-failing test, not left implicit.

**Low**

7. Window size (`checked`/`limit`) is not shown to the operator on either web surface; worth adding once D1's wording is corrected.

---

## Recommendation

**Accept RFC 121 with the changes named above**, specifically: D1's wording narrowed to what outcome 1 actually proves (§2), D2/D4 to require matching severity levels as well as matching words (§1, §2), D3 resolved in favor of a `tracing::error!` as the primary record with a best-effort audit row as secondary (§3), D4's "not fatal" made an explicit, reasoned decision rather than left open (§4), and the closure language corrected so it does not imply the underlying check itself became trustworthy (§0, §6).

**Do not accept the RFC's implicit premise that fixing the display layer resolves this control's problem.** File the Blocker (§0) as its own RFC immediately — before RFC 122–124 or anything else in this line lands, on the same reasoning the project has already applied twice (RFC 119 → 120, RFC 120 triage → 121–124): a confirmed, evidence-backed integrity defect does not wait behind unrelated work. I did not write that RFC; this package states the finding, the reproduction, and enough of a fix direction (§0's closing paragraph) for the architect to scope it.

**Entry point of this package:** `.git-exclude/review-requests/rfc-121-design-review-2026-09-29.md`
