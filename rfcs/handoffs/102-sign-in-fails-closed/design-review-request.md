# RFC 102 — independent design review request

**RFC.** [RFC 102](../../proposed/102-sign-in-fails-closed-without-audit.md), Proposed.
**Reviewer.** Mid-capability model, implementation role — authored none of it.
**Baseline.** `8fc0c62` or later.
**Scope.** Read-only. Change no code and no RFC text; report findings.

## What to check

1. **The path inventory is complete.** Find every production call that inserts a
   session row (`sessions::insert`, `insert_within_tx`, `commands::insert_session`,
   or raw SQL into `sessions`). RFC 102 lists five paths. Any sixth is a blocker.
   Report `file:line` for each call site.
2. **Each claim in *Background* is true at the baseline.** One row per checkable
   sentence: claim, `file:line`, holds or not.
3. **The TOTP replay race.** RFC 102 says `set_last_used_step` is unconditional and
   runs after `totp::verify` compared against a step read earlier, so one code on
   two pending rows can yield two sessions. Confirm or refute it by reading. If you
   can, add a test that demonstrates it. Put the test in the review package only;
   do not commit it.
4. **Implementability of L01–L04 on the existing runner.**
   - Can each transaction's contents run on `Database::class_a` with the existing
     `*_within_tx` functions?
   - Name every function that would need a `_within_tx` variant.
   - `enforce_concurrent_session_cap` reads `server_settings`. Can that read happen
     inside the transaction?
5. **R3, uniform failure.** For each path, state what response a failed transaction
   would produce under the design, and whether that is byte-identical to the step's
   ordinary failure.
6. **R4, no bypass.** Can `sessions::insert` become crate-private without breaking
   a legitimate non-sign-in caller, tests included? Say what those callers would use.
7. **The two open questions.** Give your view on each, with reasons, or say it is
   outside what this role can adjudicate.

## Return

A review-request package under `.git-exclude/review-requests/` containing:
- a findings list, severity-ranked (blocker / high / medium / low);
- the claim table for item 2;
- the call-site list for item 1.
