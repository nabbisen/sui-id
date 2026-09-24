# RFC 103 implementation handoff

**Governing RFC.** [RFC 103](../../done/103-administrator-issued-account-recovery.md),
Accepted 2026-09-17. **Implementer.** Mid-capability model.
**Reviews.** In [`../102-authentication-fails-closed/`](../102-authentication-fails-closed/README.md)
(shared with RFC 102).

## Order

| Stage | Content | Prerequisite |
|---|---|---|
| **1** | **D13** (U10 guarded consumption; in-transaction re-read: active, not deleted, `source = local`; `request_reset` refuses non-local users with the neutral response) and **D10 for the existing email path** (fragment link, static script, token text in the email, `GET ?token=` refused) | acceptance. **Dispatched now**: live defect, owner-authorized 2026-09-17 |
| 2 | **D12** — `sui-id admin reset-mfa` (U07 as system principal) | acceptance |
| 3 | Migration (`issued_via`, `issued_by`, `revoked_at`), **U37**, **D3** invalidations in U02, U04, U09, U10, U11; U10 `origin` | RFC 102 Part B Implemented |
| 4 | CLI `issue-recovery-link` | stage 3 |
| 5 | Web operation (D5, D6, D8, D10 response headers) | stage 3; RFC 102 B7 and B4 |
| 6 | D7 notices, account-page line; retire U06; matrix, manifest, docs, threat model | stages 3–5 |

## Stage 1 — landed `0fe9e3a`, 2026-09-17

Reviewed, and committed as one commit. The password-policy refusal re-shows the
form, which is accepted: the link is still good at that point.

## Stage 3 — landed `93e4f50`, 2026-09-22

Reviewed, and committed as one commit. Accepted: `origin = web` (the RFC text
said `admin`; the RFC is corrected to match the column), `issued_by ON DELETE
CASCADE`, a throttled attempt writing nothing, and the two equivalent mutants
with their reasons.

### Rulings on stage 3's findings

1. **The SMTP gate on `/reset-password` goes** (finding 1). `GET` and `POST
   /reset-password` no longer require active SMTP; `/forgot-password` keeps the
   gate, because it sends mail. Measured: without SMTP the completion page
   answers 404 today, so an administrator- or operator-issued link could not be
   redeemed on exactly the instances RFC 103 exists for. With SMTP off no
   email-origin token can exist, so opening completion exposes nothing new.
2. **`validate_token` is removed** (finding 2). D10 took away its only caller,
   and the store's guarded consume is the authority. Remove the function, its
   test and the contradicting doc lines.
3. **A reason cannot forge note fields** (finding 3). Two parts:
   - **Now, in stage 4:** the entry points bound the reason to **200 characters**
     after trimming, and refuse ASCII control characters, as a typed refusal.
     That also settles finding 4 (an over-long reason must not reach the
     512-byte attribute bound and fail generically).
   - **Proposed to the owner:** escaping attribute values in the note builder,
     which is registry-wide and affects every event. Until it lands,
     `operators.md`'s `LIKE '%step_up=…%'` queries can match a forged reason —
     a false positive, never a false negative. Say so in the guide, in one
     sentence, with the note that the real fields are written last.
4. **Stage 5's threat-model and dangerous-operations updates** stand as the
   implementer left them: they belong to the stage that adds the surface.

## Stage 4 — landed `b3ee7de`, 2026-09-22

Reviewed, and committed as one commit. Accepted:
- **The byte cap added to the reason.** The ruling's 200-character bound was not
  enough: 200 Japanese characters is 600 bytes, over the audit attribute's
  512-byte limit, which is the generic failure the ruling existed to prevent.
  The implementer measured it and capped both, as the same typed refusal. The
  ruling is amended to "200 characters **and** 512 bytes".
- **An administrator with no second factor is told so,** instead of being sent to
  a step-up page they can never pass. The dispatch's wording would have been a
  dead end.
- **The dead display condition was removed** rather than left untestable.

**For the owner: the Japanese and Chinese strings are the implementer's own** and
want a native read, especially the handover guidance and the refusal messages.

## Stage 5 — landed `37c10c2`, 2026-09-22

Reviewed, and committed as one commit. 40 of 40 hunk hashes verified, nothing
unclaimed. Gates re-run on the candidate: fmt, G07, G07b, G01, G10a, G10b, G11,
G12, G13 (56 matrix entries, 56 source literals), G15 all exit 0; 837 tests pass
on the workspace and 843 with all features, 0 failed, 3 ignored — both matching
the package's own counts.

Accepted, each flagged by the implementer rather than folded in silently:
- **The D7 gate fixes a live defect, not only an absent feature.** Before it,
  `consume_and_reset_password` mailed the target's stored address on *every*
  origin, so a web- or CLI-issued link's completion already sent the notice D7
  forbids. Shipped in stage 3; corrected here.
- **`DASHBOARD_IMPORTANT_PREFIXES` swaps `user.reset_password` for
  `user.recovery_link.issued`.** Beyond the dispatch. Endorsed: removing the
  dead prefix is forced by the retirement, and without the new one the
  dashboard's "important events" filter would quietly stop covering the most
  dangerous operation an administrator has.
- **The expiry test, added rather than reported.** Nothing anywhere exercised
  `expires_at` — every D3 test kills a token by a competing event. Since T4 and
  T7 name expiry as their control, filing it as a gap would have left
  prerequisite 5 unmet for two threats. Origin-agnostic, correctly: the guard
  has no origin branch.

**Closure, for `@nabbisen`.** Prerequisites 1–4 and 6 are met. Prerequisite 5 is
met for 10 of 13 threats; T2's control is deletion (grep-proof, no code to test),
T8 is procedural by construction, and T12 is the same compare-and-swap statement
tested at stage 1 under a different origin. Prerequisite 7 has **no eligible
reviewer in this team**: RFC 103's own clause bars its author, which is the
architect role, and RFC 000 bars the implementer. The same position RFC 102
reached, which `be829c4` resolved as an owner-carried judgment.

## Stage 5 — dispatched 2026-09-22: notices, the account page, U06's retirement, closure

**Baseline.** `b3ee7de` or later. This is RFC 103's last implementation stage.

**5a — D7 notices.**
- **At completion**, the existing notice goes to the address that received the
  link — that is, the **email origin only**. A web- or CLI-issued link sends no
  notice, because no address is proven yet: RFC 101 introduces verification, and
  RFC 101's implementation adds the verified-address notices for both events.
  Record that in RFC 101's handoff when it exists, not here.
- **At issuance**, nothing is sent, for the same reason. Say so in the guide, so
  an operator is not left expecting mail.
- Do not gate any of this on SMTP being configured beyond what the mailer
  already does.

**5b — the account-page line.** `/me/security/overview` shows the most recent
recovery event for the signed-in user: when a link was issued for them, and when
one was completed. It is read from `audit_log` (`user.recovery_link.issued`,
`auth.password.reset_completed`), shows no token and no actor's name, and says
"an administrator" or "the host operator" by `via`. A user with no such event
sees nothing.

**5c — retire U06.** Remove `identity::admin::users::reset_user_password`, the
store command, its descriptor and tests, the manifest row and the matrix row, and
`user.reset_password` from `docs/src/reference/audit-events.md`. The design
review's call-site list is the checklist. Confirm by grep that nothing names it.

**5d — closure.** `docs/threat-model.md` gains a dated entry, as RFC 102's did:
an administrator never learns a password; the link is single-use, expiring,
hashed at rest and revoked by any competing change; the web path refuses
administrator and non-local targets and needs a fresh step-up from an
administrator who holds a second factor; the CLI's authority is the host key; the
token never reaches a URL the server sees, a log or the audit row; and the
residuals (an administrator can still take over a non-administrator account with
no second factor, and the handover channel is procedural). Then a closure table,
prerequisite by prerequisite, in the RFC 102 stage 8 form.

**Evidence.** Tests for each notice rule (email origin sends, web and CLI do
not), the account-page line for each event and its absence, a grep-proof that U06
is gone, and the gates.

## Stage 4 — as dispatched

**Baseline.** `93e4f50` or later. This stage is the RFC's step 3 and step 4
together, because both are thin callers of stage 3's data path.

**Scope.**
- **The rulings above:** the SMTP gate, `validate_token`, and the reason bound.
- **Web (D5, D6, D10).** A dangerous-operation surface on the user detail page:
  confirm screen, `_confirmed=1`, fresh step-up, a required reason. It calls
  `recovery_link::issue_as_admin`. The response **renders the link and the token
  once**, with `Cache-Control: no-store` and `Referrer-Policy: no-referrer`, and
  never a redirect carrying either. `StoreError::StepUpRequired` maps to the
  step-up redirect; each `RecoveryRefusal` maps to its own message.
- **CLI.** `sui-id admin issue-recovery-link --config PATH --username NAME
  --reason TEXT`, printing the link and the token to stdout only, exiting
  non-zero on each refusal with its message. Add it to `--help`, coordinating
  with `roadmap/cli-help-completeness/`.
- **The link** is `<server.issuer>/reset-password#t=<token>` on both paths.
- **i18n** in en, ja and zh_hans for every new string.
- **Docs.** `dangerous-operations.md` gains the operation and its handover
  procedure (D11); `operators.md` gains the CLI command and the query caveat
  above.

**Evidence.**
- End to end on the web: issue, then complete at `/reset-password`, with SMTP
  **off**; the second use is refused.
- The same for the CLI, with the real binary.
- Refusals: admin target, self, non-local, disabled, deleted, missing reason,
  over-long reason, control characters, the sixth issuance within an hour.
- No step-up, and stale step-up: refused and redirected.
- The token appears in no log line at any level, in no redirect and in no
  request URI, with `log.access_log = true`.
- Headers asserted on the issuance response.
- **Mutation:** the confirm gate, the step-up gate, the reason bound and the
  no-store header, one at a time; each caught.
- **Build and gates:** fmt, both clippy scopes, the test count, MSRV 1.95, G12,
  G13, G15, G10a, G10b.

## Stage 3 — as dispatched

**Baseline.** `4057e6a` or later.

**Scope** — RFC 103 D2, D3, D9 (data path only; no CLI or web surface yet):
- **Migration** on `password_reset_tokens`:
  - `issued_via TEXT NOT NULL DEFAULT 'email'`, CHECK in `email`, `web`, `cli`;
  - `issued_by` (nullable user ID), with a CHECK that it is non-NULL exactly when
    `issued_via = 'web'`;
  - `revoked_at` (nullable).

  `count_active_for_user` excludes `consumed_at` and `revoked_at`.
- **U37 — issue recovery link** (sealed Class A). Two entries, following RFC 102
  stage 7's signature pattern:
  - the web entry takes the admin actor and `SessionId`, and carries B4 `step_up`
    evidence, which **must be `fresh`** (D6); `not_required` rolls back with
    `StepUpRequired`;
  - the system entry is for the CLI, with `not_applicable`.

  In one transaction:
  1. re-read the target: it exists, is active, not deleted, and
     `source = local`; on the web entry it is also not an admin and not the
     issuer (D5);
  2. revoke every outstanding token for the target (`revoked_at`);
  3. insert the new token hash with `issued_via` and `issued_by`;
  4. write `user.recovery_link.issued` with actor, target, the reason as note,
     `via`, `expires_at`, `invalidated` (count) and `step_up`.

  It returns the plaintext token to the caller, which never logs it. The token
  is generated outside the transaction and only its hash is stored.
- **D8 throttle, counted from the database.** The web entry allows five per
  issuing admin per rolling hour (`issued_by`). The CLI entry allows five with
  `issued_via = 'cli'` per rolling hour. A refusal writes nothing and returns a
  dedicated error.
- **D3 invalidations**, each inside its own existing transaction:
  - U09 (self password change): revoke the user's outstanding tokens;
  - U10 (completion): revoke the user's *other* outstanding tokens;
  - U02 (disable) and U04 (delete): revoke all tokens.
  - **U11 (email change):** the command does not exist yet. It arrives with RFC
    101, so record the requirement in RFC 101's handoff rather than build it
    here. If you find that an email change exists in production, stop and report.
- **U10 `origin`.** `auth.password.reset_completed` gains `origin` (`email`,
  `web`, `cli`), read from the consumed token's `issued_via`.
- **Retire U06 now?** No: its retirement is RFC 103 step 6. Leave it.

**Evidence.**
- U37 through the store: happy path on the web and the system entry.
- Each D5 refusal writes nothing: admin target, self, non-local, disabled,
  deleted.
- `not_required` evidence on the web entry rolls back.
- Throttle: the sixth issuance is refused, for web and for CLI.
- Issuance revokes earlier tokens, and a revoked token is refused at completion.
- Each D3 invalidation, per command.
- `origin` on completion, for each origin.
- Injected append failure: no token, no revocation.
- **Mutation:** the D5 refusals, the `fresh` requirement, the throttle and each
  invalidation, one at a time; each is caught.
- **Build and gates:** fmt, both clippy scopes, the test count, MSRV 1.95, G13,
  G15.

## Stage 2 — landed `6aca4b3`, 2026-09-17

Reviewed, and committed as one commit. Accepted:
- **`via = cli`** is emitted only on the CLI path, so the web event is unchanged.
- **U07's actor becomes `Optional`,** which a permitted command requires. The web
  path's actor is held by `admin_reset_mfa` using `for_authorized_actor`; a test
  and mutation R1 prove it.
- **The CLI resets any non-deleted user.** Filesystem authority already implies
  full control.

**Stage 3 may now start.** RFC 102 Part B's code is complete at `042b6dd`: B7, L05 and L06, B3, and B4 with its `not_applicable` form. RFC 102 stage 8 (documentation) does not gate RFC 103's code. Its dispatch follows the order table above: migration, U37, the D3 invalidations and U10 `origin`.

## Stage 2 — as dispatched

**Baseline.** The commit that adds this section, or later.

**2a — two follow-ups from stage 1's review.**
- **Log levels.** An unknown, used or expired token is an ordinary user outcome,
  which anyone can trigger and repeat. Do not log it at error level. Log the
  mapped `InvalidCredentials` at **info**, with `request_id` and no token. Store
  and internal errors stay at error level. Test both levels, the same way the
  stage 1 log test captures them.
- **No caching.** Responses from `GET /reset-password` and `POST /reset-password`
  carry `Cache-Control: no-store`. The re-rendered form holds the token in a field
  value. Test the header on both.

**2b — D12, `sui-id admin reset-mfa`.**
- `sui-id admin reset-mfa --config PATH --username NAME --reason TEXT`.
- U07's declaration becomes `system_principal: permitted`, and the CLI adapter
  constructs its context with `for_system_actor`. The web handler path is
  unchanged.
- Refuses:
  - an unknown user;
  - a deleted user;
  - an empty reason.
- **Event.** `mfa.admin_reset` with `via = cli`, and the actor absent. RFC 102 B4's
  `not_applicable` evidence arrives with stage 7 of RFC 102. Do not pre-empt it;
  add only what U07's descriptor requires today.
- **Help text.** Add the subcommand to `--help`, coordinating with
  `roadmap/cli-help-completeness/` if that package has landed.
- `docs/src/guides/operators.md`: the lost-every-factor procedure for a sole
  administrator.

**Evidence.**
- 2a: tests for both log levels and for the header.
- 2b tests:
  - the CLI removes TOTP and passkeys for a named admin, and one event is
    committed with `via = cli`;
  - an injected append failure leaves the factors in place;
  - the web path still requires an admin session and step-up;
  - each refusal writes nothing.
- Mutation on the system-principal gate: the web path must not reach the system
  principal.
- **Build and gates:** fmt, both clippy scopes, test count before and after,
  MSRV 1.95, G13.

**Baseline.** The commit that adds this file, or later.

**The live defect.** An LDAP or federated user can request a password reset
(`request_reset` looks up by email and checks only disabled or deleted). Completing
it (U10) sets a local password that `login_with_mfa` accepts, which bypasses the
directory. A user disabled in LDAP keeps access. The reset token also travels in
the query string of `GET /reset-password?token=…`, so it reaches proxy access logs
and the trace span.

**Scope.**
- **`request_reset`.** A user whose `source` is not `Local` gets the same neutral
  response as an unknown address, and no token is issued. Emit the same Class-B
  event an unknown address emits; do not add a distinguishing one.
- **U10.** The token is consumed with the guarded update (`consumed_at IS NULL AND
  expires_at > ?`, rows = 1). The token row and the user are re-read inside the
  transaction: active, not deleted, `source = Local`. Any failure rolls back and
  gets the existing invalid-link response.
- **The link.** Use `<server.issuer>/reset-password#t=<token>`; `server.issuer`
  replaces `smtp_config.base_url` as the link base.
  - The email also carries the token text on its own.
  - The completion page loads a static script under `crates/sui-id/static/`,
    which moves the fragment into a hidden field and calls
    `history.replaceState`.
  - Without JavaScript, the page shows a paste field.
  - The form POSTs the token.
  - `GET /reset-password?token=…` renders the "request a new link" page, and the
    token is not processed.
- **Uniform failure.** Every completion failure gets the existing invalid-link
  response. Log the cause at error level (the R11 1b pattern), with no token.

**Evidence.**
- **Directory bypass.** An LDAP-sourced user (use `InMemoryUserSource`, as the
  design review did): forgot-password issues nothing and returns the neutral
  response. A token minted directly for that user is refused at completion, and
  no credential row is written.
- **User state.** A user disabled between issuance and completion is rolled back.
- **Concurrency.** Two completions of one token give exactly one credential
  change.
- **Token never in a server-visible URL.**
  - With `log.access_log = true` and a captured subscriber, the token appears in
    no log line and in no request URI.
  - `GET ?token=` is refused.
  - The script is served from `static/` and passes the CSP (`script-src 'self'`).
- **Mutation.** Remove the `source` check, the `consumed_at IS NULL` guard and
  the in-transaction re-read, one at a time; each removal is caught.
- **Build and gates.** fmt, both clippy scopes, test count before and after,
  MSRV 1.95, G12 (en, ja and zh_hans strings), G13.
- **Docs.** Update `docs/src/guides/operators.md` wherever it describes the reset
  link.

---

## Closure assessment — 2026-09-24

**Assessed by.** High-capability model, architect / deputy PM — **not the
implementer of any stage**, which is the bar RFC 000 sets ("the implementer
cannot be the sole approver of a security-sensitive design or its closure
evidence"). Routing per `ROADMAP.md` §S1: an implementation is reviewed by the
role that specified it.
**Tree.** `420fbef`.

### The evidence was re-verified at this commit, not taken from stage 5

Stage 5's control-by-control table was assessed on 2026-09-22. RFC 115 stage 3
then changed the **fixtures** of several U37 tests that table cites — a freshly
seeded user is now a *provisioning* target, so tests about the ordinary
five-per-hour limit had to seed a live one. The tests still prove their
controls, but a closure record citing them had to be re-checked rather than
inherited.

**Re-run at `420fbef`: all 33 tests the stage 5 table cites exist and pass.**
No cited test was renamed, deleted or weakened; the three whose fixtures changed
(`u37_web_issues_a_link_and_writes_one_event`, `u37_web_refusals_write_nothing`,
and the sixth-issuance pair) assert the same controls against a target that is
now explicitly live.

### Prerequisites

| # | Prerequisite | Status at `420fbef` |
|---|---|---|
| 1 | Issue on the web | met |
| 2 | …and through the CLI | met |
| 3 | The user sets their own password with it | met |
| 4 | No path lets anyone but the holder choose or learn a password | **met**, under the owner's recorded reading of 2026-09-24, *and* materially: RFC 115 removed U01's password parameter, so setting a password on a user's behalf is now a compile error |
| 5 | Every threat has a test that fails when its control is removed | **met.** T2's control was deletion, evidenced by a grep-proof and called out at the time as the weakest row; RFC 115 has since made it a **compile error**. T8 is procedural and the RFC says so. T12 is the same compare-and-swap statement tested at stage 1 — re-checked here: RFC 115 stage 3 did **not** touch `mark_consumed_within_tx`, so that disclosure still holds |
| 6 | `docs/threat-model.md` states the properties | met, and extended 2026-09-24 by RFC 115's entry |
| 7 | Independent closure review accepts the evidence | **met** — this assessment |

### Three things this closure is made with in view, not in spite of

Recorded because a closure that hides what is still true is worth less than one
that does not.

1. **RFC 103's own flow can be denied by a stranger.** A link lives 30 minutes;
   an unauthenticated attacker who knows a username can lock the account for up
   to 24 hours, renewably, and completing a reset clears neither the counter nor
   the lock. So the recovery this RFC provides can be made to fail before the
   holder can use it. Stated in `docs/threat-model.md` and owned by **RFC 118**.
2. **Its audit story rests on an unescaped note format.** `last_note_field`
   depends on "the real fields are written last", which is a convention rather
   than a property of the data. False positives only, never false negatives.
   Owned by **RFC 105**.
3. **Its stage 5 evidence cited `ci/audit-coverage-matrix.md` as though the
   whole file were verified.** It was not — RFC 116's review later disproved
   twelve rows. Checked here: **RFC 103's own rows were not among them**; both
   `user.recovery_link.issued` and `auth.password.reset_completed` are backed by
   sealed descriptors and still read `A` without qualification. The evidence
   holds; the confidence expressed in it at the time did not.
