# RFC 103 implementation handoff

**Governing RFC.** [RFC 103](../../accepted/103-administrator-issued-account-recovery.md),
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

## Stage 2 — landed `6aca4b3`, 2026-09-17

Reviewed, and committed as one commit. Accepted:
- **`via = cli`** is emitted only on the CLI path, so the web event is unchanged.
- **U07's actor becomes `Optional`,** which a permitted command requires. The web
  path's actor is held by `admin_reset_mfa` using `for_authorized_actor`; a test
  and mutation R1 prove it.
- **The CLI resets any non-deleted user.** Filesystem authority already implies
  full control.

**Stage 3 waits** for RFC 102 Part B to be Implemented, which needs RFC 102
stages 6 and 7.

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
