# RFCs 102 and 103 — confirmation review of the resolutions

> *Tracked copy, 2026-09-17, of the implementation role's review package, made at
> acceptance so RFC 000's durable review reference is repository-visible. Content
> unchanged except this note and one relative link. The throwaway tests it mentions
> were not tracked.*

**Date:** 2026-09-16
**Request:** [`design-review-request.md`](design-review-request.md) §Confirmation review of the resolutions (items 17–19)
**Reviewer:** Mid-capability model, implementation role; the author of the first review
**Baseline:** `36a5e26` (the request requires `323c90a` or later)
**Scope:** Read-only. No files outside `.git-exclude/` changed.

---

## 0. Verdict

The blocker and every high finding from the first review are addressed in the RFC text, and the three live defects are recorded, not claimed fixed.

The resolutions introduce **three high findings** of their own:
- **N1** — B7's password re-entry is an unthrottled password oracle.
- **N2** — L07's per-row count is bypassed by minting new pending rows.
- **N3** — B4's required attribute collides with the CLI branches RFC 103 adds.

Two failure-table rows do not match what the handlers can return (N6). These should be settled before acceptance.

---

## 1. Findings on the new design, ranked

### High

**N1 — [RFC 102 B7] Password re-entry for first-factor enrolment becomes a password oracle for a stolen session.**
- B7: "a local user: the current password, re-entered on the enrolment form and verified by the handler." A thief holding a session cookie for a user with no second factor can submit password guesses to the enrolment form.
- Nothing in B7 routes that verification through U22's counting or the `Login` rate-limit bucket. A handler-side `verify_password` has neither.
- Today the same thief cannot test passwords at all without going through `login_post`.
- **Needed:** B7 verification counts a wrong password exactly as U22 does, uses a rate-limit bucket, and returns one uniform failure. Alternatively, it reuses step-up's L06 per-session count and revocation, which fits "a signed-in session guessing" better than account lockout.

**N2 — [RFC 102 A8 / L07] The per-pending-row failure count is bypassed by minting new pending rows.**
- L07 counts on the pending-MFA row and deletes it at 5. Each correct password submission mints a new row (`issue_pending_mfa`, `crates/sui-id-core/src/authn/session.rs:198-199`), and L01 or the password step also clears the password counter (`clear_lockout`, `:195`).
- An attacker who knows the password therefore gets 5 second-factor guesses per password submission, bounded only by the per-IP `Login` bucket. That is A8's "same guess space as step-up" without step-up's bound.
- **Needed:** count second-factor failures per **user** (or per user within a window), not per row. Or make a new pending row inherit the user's outstanding failure count, and state what finally ends the guessing (a lock, or a revocation of all pending rows).

**N3 — [RFC 102 B4 × RFC 103 D9/D12] A required `step_up` attribute cannot be computed on system-principal branches of the same commands.**
- B4 makes `step_up` a **required** descriptor attribute of every gated sealed command, derived from a session row. But:
  - **U07** (admin MFA reset) is gated on the web, and RFC 103 **D12** proposes running U07 from the CLI as a system principal, with no session.
  - **U37** has a web branch and a CLI branch; D9 already says "RFC 102's `step_up` evidence **on the web**".
  - **K01** is declared `system_principal: permitted` for "ops/CLI/scheduled" triggers (`crates/sui-id-store/src/commands.rs:57-61`), although today its only caller is the gated web handler.
- A required attribute with no session to read either blocks those branches or tempts a fabricated value.
- **Needed:** a third, closed form bound to the context constructor (e.g. `step_up: not_applicable` with reason `system_principal`, constructible only through `for_system_actor`), or separate commands for the session and CLI branches.

### Medium

**N4 — [RFC 102 L02/L05 vs L07/L06] A correct factor whose transaction fails must not count as a failure.**
- Part A says so for the password ("the failure counter does not advance, because no wrong credential was presented"). No equivalent sentence exists for the second factor (L02 → L07) or step-up (L05 → L06).
- If a handler runs the failure command whenever the success command errors, then during a partial outage a user with the right code can lose their pending row after 5 tries, or have their session revoked (L06).
- B5's response uniformity holds, but the state diverges. State that a failed L02/L05 does not run L07/L06, and test it.

**N5 — [RFC 102 L02; OQ3] L02 does not say whether it sets freshness.**
- Today every second factor at sign-in sets `last_step_up_at = now` (`crates/sui-id-core/src/authn/mfa.rs:250`, `:299`). The L02 row lists its writes without `last_step_up_at` or the new `last_step_up_method`.
- OQ3 recommends no freshness for a recovery code, but the row does not carry it, and B4 derives `method` from `last_step_up_method`.
- **Needed:** the row should state, per method: TOTP and WebAuthn set freshness and method; recovery code sets neither, if OQ3 is ruled that way.

**N6 — [RFC 102 failure-response table, item 19] Two rows describe responses the handlers cannot produce; details in §3.**
- **Path 3 (WebAuthn at sign-in):** `HttpError::html(Unauthenticated)` is a **303 redirect to `/admin/login`**, not a "401 error page" (`crates/sui-id/src/http/errors.rs:266-270`).
- **Step-up WebAuthn:** a **fetch endpoint** that returns JSON 400 `{"error":"step_up_failed"}` to `static/step-up-webauthn.js`. "The same 400 step-up page" would break that script.

**N7 — [RFC 102 A7] A7 names L01 only; path 4 has the same shape.**
- `try_login_with_cascade` creates the external-source session inside the cascade (future L03). `login_post`'s role check for an admin-only `next` runs afterwards (`crates/sui-id/src/http/handlers/admin/auth.rs:286-303`).
- A new shadow user's role is known before the transaction: it is always `user`.
- A7 should cover L03: refuse an admin-only `next` before the cascade runs.

**N8 — [RFC 103 D3] The invalidation list misses U10 and U11.**
- D3's prose covers "completing any password change". Step 2 names only U02, U04 and U09, and U10 today consumes only its own token. Say explicitly whether U10 also invalidates the user's other outstanding tokens.
- **U11 (email change)** is not listed. A token mailed to the **old** address stays valid for its 30 minutes after the address changes. That is the case where the old mailbox may belong to someone else.

**N9 — [RFC 103 D10] The no-JavaScript fallback strands the email origin.**
- D10: "Without JavaScript, the page shows a field to paste the token. The CLI and web issuance screens show the token on its own beside the link." The **email** carries only the link. A user without JavaScript cannot paste a token they were never shown separately.
- Either the email includes the token text too, or the fallback is scoped to admin-issued links and the email path's residual is stated.
- **Implementability note:** the CSP is `script-src 'self'` (`crates/sui-id/src/http/security_headers.rs`), so the fragment-moving script must be a static file under `crates/sui-id/static/`, not inline.

**N10 — [RFC 102 B7] Session age is weaker than a re-authentication that already exists for external users.**
- LDAP users have a password: re-binding against their user source proves the factor without a local password.
- Federated users can be re-authenticated upstream (`prompt=login` / `max_age=0`).
- The stated residual (a thief within five minutes of sign-in) would then apply only when re-authentication is unavailable.

### Low

- **N11 — [RFC 102 B4]** "If the session is no longer fresh at commit … the command rolls back." Give that rollback a defined response (redirect to step-up, as the gate does). Otherwise it surfaces as a store error.
- **N12 — [RFC 103 D10] Two residuals of the fragment link:**
  - The navigation that opens it records the full URL, fragment included, in browser history before `history.replaceState` runs.
  - Some mail link-rewriting services drop fragments.

  Both are worth one sentence.
- **N13 — [RFC 103 D3]** "Invalidate" is undefined: setting `consumed_at`, a new column, or deletion. It changes `count_active_for_user` and D9's issuance-to-completion join.
- **N14 — found while checking N6, pre-existing:** the sign-in WebAuthn script ignores `/admin/login/webauthn/complete`'s response and always navigates to `/admin` (`crates/sui-id/static/webauthn.js:168-170`). A failure shows as a redirect to the login page, and a pending `next` (e.g. an OIDC authorize) is dropped.

---

## 2. Item 17 — each finding of the first review

"Recorded" means the RFC states a live defect without claiming to fix it now.

| Finding | Where | Status | Resolving text (quoted) |
|---|---|---|---|
| **B1** stolen session enrols a factor | RFC 102 B-F7, **B7**, step 5, tests | **resolved**, with new gaps N1, N10 | B7: "Registering a passkey, regenerating recovery codes and enrolling TOTP are step-up-gated whenever the user already has any second factor." |
| **H1** most gated actions not Class A | B-F8; B4 scope; *Security considerations* | **resolved** | "**B4 applies only to sealed commands.** Each of the eight actions in B-F8 gains the attribute when RFC 094 converts it. Until then its row stays best-effort, and this RFC says so" |
| **H2** replay races; blob CAS; backward step | L02 row; *Failure handling*; concurrency tests | **resolved** | "the guard is a compare-and-swap on the ciphertext, `UPDATE … SET recovery_codes_enc = ?new WHERE user_id = ? AND recovery_codes_enc = ?old`"; "The guard also stops the stored step moving **backwards**" |
| **H3** returning LDAP user | *Findings recorded while scoping* | **recorded** | "**A returning LDAP user cannot sign in** (H3). … Path 4 works once per user." Listed under "each needing its own fix now". No fix claimed. |
| **H4** federation MFA fail-open; disabled users | L04 row; *Findings* | **resolved in design; live defect recorded** | L04: "**The local-MFA decision comes from a successful read**; a read error refuses the sign-in"; *Findings*: "L04 closes this; the live defect needs a fix before L04." |
| **H5** external users get local passwords | RFC 102 *Findings*; RFC 103 T10, **D13** | **resolved in design; live defect recorded** | RFC 103 D13: "re-reads the user inside the transaction: active, not deleted, and `source = local`"; "`request_reset` refuses non-local users the same way". Fixed in RFC 103 step 1, not claimed fixed. |
| **H6** reset token in URL | RFC 103 **D10** | **resolved**, with N9, N12 | "The link is `<issuer>/reset-password#t=<token>`. A URL fragment is not sent in the HTTP request" |
| **M1** byte-identical; 500s | RFC 102 failure table | **partly resolved** | Normalisation defined: "Tests normalise exactly those two and compare the rest byte for byte". Two rows are wrong (N6). |
| **M2** evidence computed by the command | B4 | **resolved**, with N3, N11 | "**The command computes it, not the gate.** A gated command takes the session ID as an input, re-reads the session row inside its own transaction" |
| **M3** session nobody holds | A7 | **partly resolved** (N7) | A7: "is refused **before** L01, from the role read outside the transaction" |
| **M4** second-factor failures uncounted | A8, L07 | **partly resolved** (N2) | A8: "After **5**, the pending row is consumed and the sign-in must start again from the password." |
| **M5** unguarded token consume | RFC 103 D13 | **resolved** | "`UPDATE … SET consumed_at = ? WHERE id = ? AND consumed_at IS NULL AND expires_at > ?`; zero rows → roll back" |
| **M6** U26 not a command | L03 row | **resolved** | "U26 is listed in the manifest as an implemented command, but none exists …; L03 subsumes it and the manifest row is corrected." |
| **M7** sole administrator without factors | RFC 103 **D12**, T13, OQ 4 | **routed to the owner, as the review advised** | "**Owner ruling pending.** … Recommended: a CLI operation, `sui-id admin reset-mfa`" (see N3 for the command-shape consequence) |
| **M8** CLI throttle; outstanding-token budget | RFC 103 *The CLI operation*; D3 | **resolved** | "D8's CLI limit is counted from the database (tokens with `issued_via = 'cli'` in the last hour)"; "an admin-issued link never coexists with the email path's `MAX_OUTSTANDING_TOKENS_PER_USER` (3) budget" |
| **L1** `step_up.rs:89-90` doc comment | — | **open** (implementation-level; fits step 6's B3 refusal) | not mentioned |
| **L2** signing-key hard delete has no manifest row | B-F8 | **recorded** | "signing-key hard delete (no manifest row at all)" |
| **L3** `audit-events.md` lists `user.reset_password` | RFC 103 step 6 | **resolved** | "`docs/src/reference/audit-events.md` (which still lists `user.reset_password`)" |
| **L4** cascade treats any DB error as "unknown locally" | — | **open**; belongs with H3's fix | not mentioned |
| **L5** migration 0032 comment | — | outside both RFCs (RFC 097 input, per RFC 098 dispatch 15) | — |
| **L6** `sessions::insert` test callers; U30 retired | *Authority for the context* | **resolved** | "`commands::insert_session` (U30 …) has no production caller and is retired with A4 … a store-side test constructor that creates a session **through L01's transaction**" |
| request-id span defect (from R11 Part 1) | *Findings* | **recorded** | "The request-id middleware holds a span guard across `.await`" |

---

## 3. Item 19 — the failure-response table against the handlers

| Step | RFC says | What the handler can return after the change | Leaks the cause? | Matches? |
|---|---|---|---|---|
| Password (paths 1, 4) | 401 login page, uniform flash | `login_post` `Err` arm: 401, fixed flash. R11 Part 1 now logs non-credential errors server-side only. | no | **yes** |
| TOTP / recovery code (path 2) | 401 challenge page, uniform flash | `mfa_challenge_post` `Err` arm: 401 with flash and CSRF token. `has_passkey` is read from the pending row, which a failed L02 leaves in place, so the body is unchanged. After L07 deletes the row, the passkey button disappears; the 5th response differs from the 4th, which is visible but reveals nothing about the code. | no | **yes** (note N2) |
| WebAuthn second factor (path 3) | "401 `Unauthenticated` error page" | `HttpError::html(CoreError::Unauthenticated)` is a **303 redirect to `/admin/login`** (`crates/sui-id/src/http/errors.rs:266-270`), and the calling script ignores the response anyway (N14) | no | **no** — the row should say "redirect to `/admin/login`" (N6) |
| Federation callback (path 5) | redirect to `/admin/login?fed_error=signin_failed` | a redirect is producible. **No template in `crates/sui-id-web` reads `fed_error`**; none of today's `fed_error=` values is rendered, so the user sees a plain login page. | no | **yes** for uniformity; note the missing message |
| Step-up TOTP (L05/L06) | 400 step-up page, invalid-code flash | `step_up::post` `InvalidCredentials` arm: 400 page. Other errors must be mapped to that arm (today `Err(other) → HttpError::html`, i.e. 500). | no | **yes**, once the mapping is added |
| Step-up WebAuthn | "the same 400 step-up page" | `step_up::webauthn_finish` is a fetch endpoint returning JSON 400 `{"error":"step_up_failed"}` for every error (`crates/sui-id/src/http/handlers/step_up.rs:309-321`), consumed by `crates/sui-id/static/step-up-webauthn.js` | no | **no** — keep the uniform JSON 400 (N6) |

**Missing rows:**
- **L07's `auth.mfa.pending_revoked` response:** the next request has no pending row, so it redirects to `/admin/login`.
- **A7's refusal.**
