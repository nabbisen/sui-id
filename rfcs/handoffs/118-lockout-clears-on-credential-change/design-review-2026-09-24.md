# RFC 118 — independent design review

**Date:** 2026-09-24
**RFC:** [RFC 118 — A credential change clears the lockout, and the user is told](../../accepted/118-lockout-clears-on-credential-change.md), Proposed
**Request:** [`design-review-request.md`](design-review-request.md)
**Reviewer:** Mid-capability model, implementation role. Authored neither the RFC nor its handoff.
**Baseline read:** `c0e0ac5` (contains `0.78.0`). **Read-only: no code, no RFC text and no `ci/` file changed.** The one thing I ran that touches code was a **scratch probe in a throwaway `git worktree`** (`item 6`), never committed and removed; `git status` on the repository is empty.
**Outcome: accept with changes, not as written.** The defect is real and reproduces on both paths. D1's core (clear the counter and the lock in the credential's own transaction) is right and buildable. But **D3 as written cannot fire, D1 as written weakens second-factor brute-force protection, and closure prerequisite 2 claims something no per-account lockout can deliver.** Details and the changes I name are in §2 and §6.

---

## 1. The claim table (items 1–6)

| # | Claim | Where | Holds? |
|---|---|---|---|
| 1 | `lockout_backoff` reaches 24 h at ten failures and stays | `crates/sui-id-core/src/authn/session.rs:44-57` (`_ => 24 * 60 * 60`, then `secs.min(max_secs)`); the default cap is 24 h (`runtime/config.rs`, `MaxLockoutDuration`, `#[default] TwentyFourHours`, options 15 min–48 h) | **Yes, at the default.** An operator's `[security] max_lockout` lowers or raises the ceiling. Also true and worth stating: the counter does **not** reset when a lock lapses (`session.rs:182-188` comment; `repos/users.rs:337-356` writes `count + 1`), so **one wrong attempt per window renews the lock at the same step** |
| 2 | An active lock refuses sign-in before the password is checked | `session.rs:172-181`: the `locked_until > now` branch runs a dummy Argon2 verify, writes a best-effort audit row and returns `InvalidCredentials`; `credentials::get` is later (`:190` region). The directory-user path does the same at `http/handlers/admin/auth.rs:216-221` | **Yes** |
| 3 | U10 clears neither counter nor lock | `crates/sui-id-store/src/commands.rs:1281-1330`: read in full. It consumes the token, re-reads `is_active_local`, reads `origin`, upserts the credential, revokes sessions, refresh tokens and sibling links. **No statement touches `failed_login_count` or `locked_until`** | **Yes** |
| 4 | U09 clears neither | `commands.rs:1154-1207`: credential upsert, `revoke_outstanding_for_user`, optional session/refresh sweep. No write to either column | **Yes** |
| 5 | The only reset is a successful password verify (`clear_lockout`), which the lock prevents | **True in substance, wrong in detail.** `repos/users.rs:359-368` `clear_lockout` has **no production caller** (only a test, `commands/tests/runner/lockout.rs:183`, and three stale comments: `session.rs:184`, `registry.rs:186`, `commands.rs:1018-1023`). The real resets are **L01** (`commands.rs:2010`) and **L03** (`:2078`), both `record_password_login_within_tx` (`users.rs:376-399`, which itself refuses a locked user at `:390`), **L02** (`:2319`, `users.rs:409-416`, which also clears `mfa_failure_count`), and **U08** `admin_unlock` (`:1078`, `users.rs:457-468`, the operator CLI). For an account **with a second factor** a correct password alone clears nothing: only L02 does | **Holds; the RFC and handoff should name L01/L02/L03/U08, not `clear_lockout`** |
| 6 | The denial is real end to end | **Reproduced on both paths** (below) | **Yes** |

**Item 6, the construction.** In a scratch worktree at `c0e0ac5` I added an e2e probe (never committed, removed afterwards). Method: fail sign-in for `alice` through the real endpoint, letting each lock lapse by rewriting `locked_until` into the past (standing in for time), then one more failure so the lock is live.

- **U10.** Before: `failed_login_count=4`, `locked_until` set. A valid reset link is issued and completed (`303 → /admin/login?reset=ok`). After: **`count=4`, `locked_until` byte-identical.** Sign-in with the new password: **`401`, body carries the generic invalid-credentials text.**
- **U09.** Same starting state with a live session; `POST /me/security/password` with the correct current password (`303`). After: **count and lock unchanged.** Sign-in with the new password: **`401`.**

I did not reach a 24-hour lock in the probe: a first attempt with ten failures failed an assertion I did not diagnose (the login limiter is 10 a minute per IP, `runtime/ratelimit.rs:108`, which is the likely cause), so the reproduction uses four failures (a 60-second lock). It shows exactly the same absence of clearing; the 24 h figure is claim 1, by reading.

---

## 2. Findings, ranked

### Blocker

**B1 — D3 cannot fire under D1.** D1 clears the lock **in the completion's own transaction**. D3 says the completion flow tells a user "that the account is temporarily locked … and when it lifts." At the moment the completion flow can speak, the lock it would describe **has just been cleared**, so the message is always false. The state D3 targets ("refused immediately after setting their password") can only arise from a **re-lock after** completion, and the completion flow cannot know that. (Traced: the completion response is produced from U10's result at `handlers/forgot_password.rs:207`; nothing else runs afterwards.) D3 needs a different content, and a home: see §4 items 11–13. This blocks D3, not D1.

### High

**H1 — D1 as written weakens second-factor brute-force resistance.** `locked_until` is **shared**: the second-factor lockout L07 sets it (`commands.rs:2440-2448`, `UPDATE users SET locked_until = …`) from the separate `mfa_failure_count` (`users.rs:424-434`). D1 does not say which lock it clears, and nothing in the row says why it was set. A holder of a **reset token** (anyone with the user's mailbox) can reset the password, clear the lock, sign in with the password *they just chose*, and guess second-factor codes. The threshold is `mfa_failure_count >= 5` (`commands.rs:2377`), so if the count is left alone every wrong code re-locks: **one guess per reset**; if D1 also clears `mfa_failure_count` (as U08 does, and as RFC 102 stage 9 did for the operator), **five per reset**. Today the same attacker is stopped by the escalating window (5 min, 30 min, 2 h, … 24 h). The reset flow is throttled only per IP (`forgot_password: Limiter::new(5, 60)`, `ratelimit.rs:108-118`), and rotating IPs is free. This is the exact case second factors exist for: **password (and mailbox) compromised, second factor intact.** **Fix:** D1 clears `failed_login_count` always; clears `locked_until` **only when `mfa_failure_count < MFA_FAILURE_LOCKOUT_THRESHOLD`** (so a lock that a second-factor lockout may have set stays); and **never** clears `mfa_failure_count`. Say so in D1; test it.

**H2 — Closure prerequisite 2 overclaims.** "An unauthenticated party cannot prevent a user from signing in with a password that user has just set" is not what D1 delivers, and no per-account lockout can. After a clear the counter is 0; three counted wrong attempts (one minute of one IP's budget) re-lock at 30 s, and an attacker who attempts at each expiry keeps the account refused (the user's only window is the gap between expiry and the attacker's next attempt). What D1 changes is **who must stay active**: before, one act locked the account for 24 h and it stayed locked with the attacker asleep; after, the lock must be **continuously renewed**, and climbing from zero back to the 24 h step takes about **21 hours** of correctly timed attempts (30 s + 1 min + 5 min + 30 min + 2 h + 6 h + 12 h). That is a large, real improvement, and it is what the RFC's own "What this does not do" already says. **Reword the prerequisite to what is true:** *"a lock cannot outlive the credential change that made it moot."* And state the remaining residual in the threat model.

### Medium

**M1 — The `?reset=ok` redirect is dead.** `handlers/forgot_password.rs:207` redirects to `/admin/login?reset=ok`, but `login_get` reads only `next` (`admin/auth.rs:37-40, 79-106`; nothing reads `reset` anywhere). **Today a user who completes a reset lands on the sign-in page with no confirmation whatsoever.** This matters for D3: the natural home for a message (a flash on the login page) does not exist, and building one from a query parameter would be a public, spoofable banner, or, if derived from account state on `GET`, an enumeration oracle. The message must come from the `POST` response itself (item 11).

**M2 — The RFC and handoff name the wrong function** (claim 5): `clear_lockout` is dead code; the resets are L01/L02/L03/U08. Also the config doc comment `runtime/config.rs:195-199` says `max_lockout` stamps a `Retry-After` header "on a locked response"; **nothing does that** (`Retry-After` exists only on the per-IP rate limiter, `handlers.rs:494-549`). It is stale, but it is exactly the kind of sentence that, if someone made it true, would break D4. Say so beside D4.

**M3 — What an operator loses, and what should replace it.** Nothing in the audit trail records that a lock was lifted. `users.failed_login_count` / `locked_until` are visible only by SQL (no admin page shows them; `grep locked crates/sui-id-web` finds only the 24-hour count on the logs page), and `auth.lockout` rows remain, so the *history* survives. What disappears is the **current** state, and with it the only signal that "this account was being guessed when the reset landed". **Recommend** an optional attribute on U09 and U10, `lockout_cleared=<count>` (present only when the counter was non-zero or a lock was live), so `auth.lockout … → auth.password.reset_completed lockout_cleared=…` reads as a story. It changes two descriptors, so it is an RFC decision, not a detail.

**M4 — "No second path" needs a mechanism, not a sentence.** Give U09 and U10 **one** helper, `users::clear_password_lockout_within_tx(conn, id)` (beside `record_password_login_within_tx`), called from both closures. And extend RFC 115's `r115_s2_credentials_writers_are_the_allowlist` so that the only production writers of `credentials` outside setup and `--dev` (U09, U10) are the ones that call it (a mutation that removes one call must be caught). Putting the clear inside `credentials::upsert_within_tx` would make every future writer inherit it, but `setup` and `--dev` create new rows, and a future "import a hash" path might legitimately not want a clear; I recommend the helper plus the test.

### Low

**L1 — Pre-existing timing difference, not introduced here.** The locked branch (`session.rs:172-181`: dummy verify, one best-effort audit append) does less database work than the wrong-password branch (credential read, verify, U22 transaction, `:190-215`). The difference is sub-millisecond against Argon2's tens of milliseconds, and `r11_1c` pins status, body and metric equality, not timing. D4 is about not *changing* this; it does not. I did not measure it.

**L2 — A hijacked session gets nothing extra from U09.** U09 requires the current password (`me_security.rs` verifies it first), so an attacker with only a session cannot reach the clear. Separately, and pre-existing: a wrong *current* password on that form is never counted (documented in `operators.md`), so a session thief can guess it at the per-IP rate; D1 does not change that.

**L3 — "Any path that sets a credential" is a finite list.** RFC 115 measured four production writers of `credentials`: `setup.rs` and `--dev` (both create a **new** user, so there is nothing to clear), U09 and U10. Tie the closure prerequisite to that allowlist test.

---

## 3. Answers to items 7–10

**7. Is "any successful credential change clears the counter and the lock" the correct rule?** For the **password** counter and a **password-imposed** lock: yes. Per writer: **U10**: yes (with H1's carve-out); **U09**: yes (the user proved the current password); **`setup` and `--dev` seeding**: vacuous (new rows, counters are already 0), and clearing there is harmless; **any future writer**: unknown, which is why M4 wants a helper and a test rather than a rule in prose. The path where clearing is **wrong** is the one H1 names: a lock set by the **second-factor** lockout.

**8. D2's justification, attacked.**
- *A reset token obtained by an attacker who caused the lockout.* True as D2 says, for a **password-only** account: they now hold it. For an account with a second factor it is **not** "nothing lost": the lock was what stopped a password holder from guessing the second factor (H1).
- *An administrator-issued provisioning link.* The issuing administrator holds the plaintext token (RFC 115 D7, an accepted residual), so they can clear a lock on an account they could already take over. Nothing new.
- *A session hijack using U09.* Needs the current password (L2); nothing new.
- So D2 is true for the password lock and **false as stated for the second-factor lock.**

**9. Same transaction.** Expressible for both without a second write path. U09's closure and U10's closure both receive `tx.tx()`; the helper takes a `&Connection` like its siblings. It must (a) update only the target's row, (b) touch `failed_login_count`, `locked_until` (conditionally, H1) and `updated_at`, never `mfa_failure_count`. **The injected-failure tests** (the pattern is `u10_injected_failure_before_append_rolls_back_everything`, `runner/passwords.rs`) should assert, after `fail_before_next_append` **and** after-append: the credential hash is unchanged, **`failed_login_count` and `locked_until` are unchanged (still locked)**, the U10 token is unconsumed / the U09 sessions unswept; and on success: count 0, `locked_until` NULL, `mfa_failure_count` untouched, the event recorded.

**10. What an operator loses.** M3: the live state, not the history. Also `operators.md:1268-1347` says a lock is lifted by a completed sign-in or `sui-id admin unlock-user`; it needs a third sentence. `unlock-user` stays (it also clears `mfa_failure_count`, which a credential change deliberately does not).

## 4. Answers to items 11–14 (D3 and D4)

**11. Where can the message be shown so that it is reachable only with a valid consumed token?** In the **response to the completion `POST /reset-password` itself**, rendered directly (not a redirect), as a small "your password is set" page, `Cache-Control: no-store`, `Referrer-Policy: no-referrer`, as the recovery-link issuance page already is. The state it reads: U10 must return a **pre-clear snapshot taken in its own transaction** (`failed_login_count`, whether `locked_until` was in the future, and, per H1, whether a second-factor lock is being kept and when it lifts). The handler renders the page only on `Ok(())`, i.e. only after the token was consumed. Not a query parameter on the login page (M1: spoofable, or an oracle if derived from state), and not a cookie read by `GET /admin/login`.

**12. Is there any path by which D3's message, or a timing or response difference, becomes visible to an unauthenticated visitor?** Traced, not asserted:
- **Sign-in path (`POST /admin/login`, `/admin/mfa`, the directory path).** D3 adds no code there; D1 changes stored state only. Response and status are the same generic `401` before and after for a locked or unlocked account (`session.rs:172-215`, `admin/auth.rs:216-221`).
- **Completion `POST` with an invalid, expired, revoked or replayed token, or a valid token and a refused password** (policy or breach refusals return **before** the token is looked up, `forgot_password.rs` `consume_and_reset_password`). Each already returns a fixed refusal; none touches the snapshot. The only response that differs is the success one, which already differs from a refusal today (a `303` vs a `400`); that difference is token possession, which D3 does not widen.
- **`GET /reset-password`** renders no account state.
- **The redirect target.** If the success response stays a redirect, D3 has nowhere to speak (M1). If a flash were used, it must be a fixed enum set only by the completion response.
- **Replay.** The token is single-use (`mark_consumed_within_tx`), so the message can be shown **once**; a replay gets the invalid-link page.
So: **not reachable without a consumed token, provided the message is produced in the completion's own response.** How it should be tested is in §5.

**13. What should it say?** The state at completion is "the lock was cleared", not "the account is locked." Useful and safe wording, to a holder of a consumed token:
- *If a lock or a non-zero counter was cleared:* "Your password is set. Before this reset, sign-in for this account had been refused after repeated failed attempts; that has been cleared. If you are refused again shortly, someone may be trying passwords for your account: wait a few minutes and try again."
- *If a second-factor lock is being kept (H1):* say so, **with the time it lifts** ("sign-in with a second factor is paused until <time>"). Showing that time to a consumed-token holder is safe: they hold the token. It does imply the failure count (a 24 h window means many failures) and should be shown as a time, never a count.
- Never the number of attempts, never the source, and nothing on any surface without the token.

**14. Does clearing the lock alter any sign-in response, including timing, for any account?** **Status and body: no.** A cleared account's wrong password now takes the wrong-password branch instead of the locked branch; both return the same `401` and the same metric increment (`r11_1c`). **Audit differs** (the row's reason changes from "account locked" to a counted failure), visible only in the audit log. **Timing:** the branches were already not identical (L1) and are not made more different by D1; the account moves from one to the other. I did not measure it. So D4 holds, and the sign-in form can stay untouched.

## 5. How the boundary should be tested (for whoever builds it)

The handoff says "state how you tested it is not reachable otherwise." I would want, at minimum: (1) for every non-success completion input (no token, unknown token, replayed, expired, revoked, valid token with a too-short password, valid token with a breached password in `block` mode) the response body and status are **byte-identical to what they were before the change** (capture them on the baseline commit); (2) `GET /admin/login` with any query, including `?reset=ok&locked=1`, is byte-identical to `GET /admin/login`; (3) `POST /admin/login` for a locked account (before and after a clear) matches a wrong password in status, body and metric; (4) the message text appears in **exactly one** response class (a valid completion) and in that response **once** (a replay of the same token does not show it).

## 6. Items 15–16 and the recommendation

**15. Threats introduced or missed.**
- **H1**, the second-factor lock, is the one real new capability: a mailbox holder gets unmetered second-factor guessing.
- **Evidence loss (M3):** clearing erases the live signal that an account was under attack; the audit rows remain.
- **The DoS is not closed (H2):** an attacker can re-lock at will; the improvement is that the lock must be continuously renewed.
- **A token holder gains nothing else:** for a password-only account they already hold it; for the admin-courier case, RFC 115 D7 already accepts it.

**16. What cannot be built as described, or is better another way.**
- D3 as written (B1); D1's "counter and lock" without the second-factor carve-out (H1); closure prerequisite 2 (H2).
- **What the RFC omits and should say:** the two dead-code corrections (M2), the dead `?reset=ok` (M1), the audit attribute (M3), the single helper and its test (M4), and that `sui-id admin unlock-user` remains the operator's path for a second-factor lock.
- **The backoff schedule:** I do **not** recommend reopening it in this RFC. As a **separate** recommendation, the root of the availability problem is that the counter is keyed on the username alone; a per-(user, source) or known-device dimension would let an attacker lock only their own view of the account. That is its own design and its own RFC.

**Recommendation: accept with the changes named** (not as written):
1. **D1:** clear `failed_login_count` always; clear `locked_until` only when `mfa_failure_count < MFA_FAILURE_LOCKOUT_THRESHOLD`; never clear `mfa_failure_count`; one helper called by U09 and U10 (M4).
2. **D3:** replace with "the completion response tells a holder of a consumed token what was cleared (and a retained second-factor lock with its time)"; rendered in the `POST` response, `no-store`; U10 returns the pre-clear snapshot from its own transaction.
3. **Closure prerequisite 2:** "a lock cannot outlive the credential change that made it moot"; move the re-lock residual into the threat model.
4. **Correct** the claim-5 function names; **add** the M3 audit attribute as a decision for the owner; **add** the boundary tests of §5 to the handoff's evidence.

