# RFC 103 — Administrator-issued account recovery

**Status.** Accepted
**Accepted on.** 2026-09-17
**Approved by.** `@nabbisen`, who also ruled every open question as recommended.
**Independent design review.** [Design review 2026-09-16](../handoffs/102-authentication-fails-closed/102-103-design-review-2026-09-16.md)
by the implementation role, shared with RFC 102 (items 11–15: every credential writer, the forgot-password machinery, D3's
invalidations, U06's reachability, the threat table) and
[confirmation review 2026-09-17](../handoffs/102-authentication-fails-closed/102-103-confirmation-review-2026-09-17.md).
**Implementation owner.** Mid-capability model, by dispatch.
**Handoff.** [`../handoffs/103-administrator-issued-account-recovery/README.md`](../handoffs/103-administrator-issued-account-recovery/README.md)
**Security review.** Required
**Design prerequisites.** The owner ruling of 2026-09-16 that admin password
reset is built, as a web operation and as a CLI operation. In the owner's words:
"I care about security risk on Web version." The risk analysis below answers
that. **Independent design review** (implementation role, 2026-09-16, shared
with RFC 102): findings H5, H6, M5, M7 and M8 bear on this RFC and are resolved
below; B1 is resolved in RFC 102 B7, which D6 depends on.
**Implementation prerequisites.** Step 1 (D13 and D10 on the existing email path, which fix live defects): this RFC Accepted, and RFC 094 M2a's runner foundation (in the tree). Steps 2–6: also RFC 102 Part B Implemented, including B7, because D6 relies on step-up evidence that a stolen session cannot manufacture.
**Closure prerequisites.** An administrator can issue a recovery link on the web and through the CLI; the user can set their own password with it; no code path lets anyone other than the account holder choose or learn a password; every threat below has a test that fails when its control is removed; `docs/threat-model.md` states the resulting properties; independent closure review accepts the evidence.
**Tracks.** `ROADMAP.md` programme. This RFC is a prerequisite of RFC 101: the
§6.4/§6.5 ruling's "nobody is stranded" is true only once this RFC is
implemented.
**Touches.** Store and core: `crates/sui-id-store/src/commands.rs`, `repos/password_reset_tokens.rs` plus a migration, `crates/sui-id-core/src/identity/admin/users.rs`, `crates/sui-id-core/src/account/forgot_password.rs`; web and CLI: `crates/sui-id/src/http/handlers/admin/users.rs`, `crates/sui-id/src/cli.rs`, `crates/sui-id-web/src/pages/`, `crates/sui-id-i18n`; registries and docs: `ci/write-commands.toml`, `ci/audit-coverage-matrix.md`, `docs/src/guides/dangerous-operations.md`, `docs/threat-model.md`.
**Accountable owner and approver.** `@nabbisen`.
**RFC author / architect.** High-capability model, requirements-architect role.
**Independent security and closure reviewer.** Role independence per RFC 000 —
the reviewer must not have authored, implemented, or previously approved this
RFC; vendor is not a criterion. Design review goes to the implementation role.
The risk judgments in *Open questions* go to `@nabbisen`.

## Summary

A user who has forgotten their password and has no verified email address has
no way back in today. `reset_user_password` exists in `sui-id-core`, and as the
Class-A store command U06, but no route or CLI command reaches it (RFC 098
dispatch 14).

This RFC does **not** wire up that function. It lets an administrator choose a
password, which is the most dangerous possible shape for this capability.
Instead:
- **An administrator issues a single-use, short-lived recovery link.** The user
  opens it and sets their own password. The administrator never chooses, sees or
  knows it.
- **The link resets the password only.** A second factor still has to be passed
  at the next sign-in.
- **The web operation is limited:** it cannot target an administrator or the
  issuer, and the issuer must hold a second factor.
- **The CLI operation is the last resort.** It works for any account, including
  the sole administrator. Its authority is filesystem access to the key, the
  same as `admin unlock-user`.

U06 is retired: a capability that contradicts this design does not stay in the
tree.

## Threats — what a web recovery operation makes possible

| # | Threat | Control |
|---|---|---|
| T1 | A stolen admin session takes over accounts, and through OIDC every relying party they use | D4, D5, D6, D8, D9 |
| T2 | An administrator impersonates a user: sets a password they know, acts as the user, and the record shows the user | D1 |
| T3 | One administrator captures another administrator's identity, to act under that name or escalate | D5 |
| T4 | The link is intercepted in the handover channel (chat, email, ticket) | D2, D3, D11 |
| T5 | The takeover is silent; the user never learns of it | D7 |
| T6 | Recovery link plus MFA reset gives a full takeover of an MFA-protected account | D4, D6, D7, D9 |
| T7 | A link is replayed or kept for later | D2, D3 |
| T8 | An attacker phones the helpdesk pretending to be the user | D11 (procedure; not solvable in code) |
| T9 | The link leaks through logs, browser history, caches or `Referer` | D10 |
| T10 | An external-source (LDAP or federated) account gets a local password, bypassing the directory | D5, **D13** |
| T11 | A link issued before a user is disabled, deleted or changed still completes | D3, D13 |
| T12 | One token completes twice under concurrency | D13 |
| T13 | A sole administrator who has lost every second factor has no way back in | **D12** |

## Design

**D1 — nobody but the account holder chooses a password.** The operation issues a
link, never a password. U06 is removed: the `sui-id-core` function
`identity::admin::users::reset_user_password`, the store command, its manifest
entry and its tests. RFC 094's inventory row U06 gains a pointer here. U35's
caller list loses U06.

**D2 — the link.** It reuses the forgot-password token machinery
(`password_reset_tokens`, migration 0015):
- 256-bit random token;
- only its SHA-256 stored;
- single use;
- expiry 30 minutes, the same as `DEFAULT_TOKEN_TTL`.

The link base is `server.issuer` for **every** origin, email included. Today the
email path uses `smtp_config.base_url`; one completion URL has one origin.

A migration adds two columns:
- `issued_via`: `TEXT NOT NULL DEFAULT 'email'`, CHECK in `email`, `web`, `cli`.
- `issued_by`: a nullable administrator user ID. The CHECK requires it to be
  non-NULL exactly when `issued_via = 'web'`.

**D3 — one live link per user.** Issuing a link invalidates every outstanding
token for that user, whether email or admin-issued, in the same transaction.
Completing any password change, disabling the user and deleting the user each
invalidate outstanding tokens too. **Today none of those three does** (design
review item 13): U09, U02 and U04 contain no `password_reset_tokens` statement,
and U10 consumes only its own token. Each gains the invalidation in its own
transaction, and so do **U10** (completing one token invalidates the user's other
outstanding tokens) and **U11** (an email change invalidates every token, because
a link mailed to the old address must not outlive the address) (N8).

**"Invalidate" means** setting a new `revoked_at` column, distinct from
`consumed_at`. `count_active_for_user` excludes both. D9's issuance-to-completion
join uses `consumed_at` only, so a revoked token never appears as completed (N13). Because issuance invalidates the rest, an admin-issued link never
coexists with the email path's `MAX_OUTSTANDING_TOKENS_PER_USER` (3) budget.

**D4 — the link resets the password and nothing else.** Completion is the
existing U10 flow at `/reset-password`: policy check, HIBP, credential swap, and
revocation of every session, refresh token and authorization code, all in one
Class-A transaction. **The second factor is untouched.** An MFA-protected account
stays protected: whoever holds the link still has to pass TOTP or WebAuthn at
sign-in. Taking such an account over needs a second, separate dangerous
operation (MFA reset). That operation is independently gated, audited and
notified.

**D5 — who the web operation may target.** It is refused, with an explicit
message and before any write, when the target is:
- the issuing administrator (self-service password change and forgot-password
  exist for that);
- **any user whose role is admin** — administrators recover only through the CLI,
  so a stolen admin session cannot capture another administrator (T3);
- a disabled or deleted user;
- a user whose source is not local (T10; the rule `reset_user_password` already
  enforced for RFC 005).

The CLI operation has the same refusals **except** the admin-role and self
refusals, because its authority is not an admin session.

**D6 — who may issue on the web.** The full dangerous-operation contract:
- confirm screen;
- a **non-empty reason**, recorded;
- the `_confirmed=1` marker;
- fresh step-up.

In addition, **the issuing administrator must hold a second factor**. RFC 102
B4's evidence must be `fresh`; `not_required` (an administrator with no second
factor) is refused for this operation. A password-only admin session is exactly
what T1 steals.

**D7 — the user is told.** Two notices, neither of which carries a link:
- **At issuance:** to the user's **verified** address, once RFC 101 ships;
  before that, none. An unverified address may belong to someone else (RFC 101
  §2).
- **At completion:** the existing `notify_password_changed` notice, to a verified
  address, or, for the email origin, to the address that received the link (which
  that completion has just proved). **Today it goes to any address on file,
  verified or not** (design review item 12).

The user's own account page shows the most recent recovery event, so a user with
no address still sees it after signing in.

**D8 — throttling.** One administrator may issue at most **5** links per rolling
hour, and the CLI at most 5. A refusal writes nothing, returns an explicit error,
and emits a warn-level operational log line (RFC 102 C1 form).

**D9 — the audit record.** A new Class-A command, **U37 — issue recovery link**,
with event `user.recovery_link.issued`:
- **actor:** the administrator, or none for the CLI;
- **target:** the user;
- **note:** the reason;
- **attributes:** `via` (`web` | `cli`), `expires_at`, the count of tokens
  invalidated, and RFC 102's `step_up` evidence on the web.

U10's `auth.password.reset_completed` gains an `origin` attribute (`email` |
`admin` | `cli`). An operator can then join each issuance to its completion. The
audit row never carries the token or its hash.

**D10 — the link never leaks.** The token never appears in a URL the server
receives:
- The link is `<issuer>/reset-password#t=<token>`. A URL fragment is not sent in
  the HTTP request, so it reaches no proxy access log, no `TraceLayer` span and no
  server log. **The email link uses the same form**, which closes the same leak on
  the path that exists today (design review H6).
- The completion page's script moves the token into a hidden form field and
  removes the fragment with `history.replaceState`. The token is then submitted by
  POST.
- The script is a static file under `crates/sui-id/static/`, not inline, because
  the CSP is `script-src 'self'`.
- Without JavaScript, the page shows a field to paste the token. The CLI and web
  issuance screens, **and the email**, show the token text on its own beside the
  link for that case (N9).
- **Residuals, stated:**
  - The browser records the full URL, fragment included, in its history before
    `replaceState` runs.
  - Some mail link-rewriting services drop fragments; the separate token text is
    the recovery for that (N12).
- The old `GET /reset-password?token=…` form is refused, with a page telling the
  user to request a new link. It is not silently accepted, because accepting it
  would keep the leak.

Further:
- It is rendered **directly in the POST response**, never placed in a redirect
  URL. This is the opposite of the rotated-client-secret pattern
  `?rotated_secret=`, which RFC 098 dispatch 14 found and which is recorded
  below for its own fix.
- The response carries `Cache-Control: no-store` and `Referrer-Policy:
  no-referrer`.
- The link is never logged, at any level.
- The completion page is already `/reset-password`, and it must not load
  third-party resources.
- The CLI prints the link to stdout only, never stderr, and never in a log line.

**D12 — a sole administrator who has lost every second factor. Ruled 2026-09-17
(`@nabbisen`): build the CLI reset.** With RFC 102 B7, lost-authenticator recovery goes through an
administrator's MFA reset (U07). The last administrator has no other
administrator. The CLI today has no MFA reset, and D4 keeps MFA in force on the
recovery link. The design review found that such an administrator has **no path**
(M7). Recommended: a CLI operation, `sui-id admin reset-mfa --username NAME
--reason TEXT`. It runs U07 as a system principal, with the same filesystem
authority as `admin unlock-user`, and is recorded with `via = cli`. Without it,
the only remedy is restoring a backup. **U07 is declared `system_principal:
forbidden` today** (`crates/sui-id-store/src/commands.rs`). D12 changes that
declaration to `permitted`, reachable only through the CLI adapter, with RFC 102
B4's `not_applicable: system_principal` evidence.

**D13 — completion re-checks everything, and consumes once.** U10 (every origin):
- consumes the token with a guard: `UPDATE … SET consumed_at = ? WHERE id = ? AND
  consumed_at IS NULL AND expires_at > ?`; zero rows → roll back. Today the update
  has no `consumed_at IS NULL` guard, and the token row is read outside the
  transaction, so two concurrent completions both commit (M5);
- re-reads the user inside the transaction: active, not deleted, and
  `source = local`. An LDAP or federated user never receives a local password from
  any origin. **Today `request_reset` and U10 check neither**, so forgot-password
  already lets an external user set a local password that `login_with_mfa` then
  accepts, bypassing the directory (H5);
- `request_reset` refuses non-local users the same way, with the same neutral
  response it gives an unknown address.

**D11 — handover.** The operator guide states the procedure:
- deliver the link through a channel that authenticates the person, such as a
  call-back to a number already on record or in person;
- never to an address or number the requester supplies in the same request;
- deliver it immediately, because it expires in 30 minutes.

This is the only control for T8, and the guide says so.

### The CLI operation

`sui-id admin issue-recovery-link --config PATH --username NAME --reason TEXT`

- Runs U37 as a system principal (`for_system_actor(None)`, the precedent is U08)
  against the database named in the configuration.
- D8's CLI limit is counted from the database (tokens with `issued_via = 'cli'` in
  the last hour), because the CLI is a separate process.
- Builds the link from `server.issuer`, prints it once to stdout, and exits 0.
- Its authority is read access to the database and the master key, the same as
  every other `admin` subcommand.
- It works while the server is running.

## Relationship to other RFCs

- **RFC 101:** its implementation prerequisites gain "RFC 103 Implemented". The
  §6.3 note (corrected 2026-09-16) points here.
- **RFC 094:** U06 is retired and U37 added, each with a one-line pointer in the
  inventory on acceptance. The precedent is RFC 095.
- **RFC 102:** D6 consumes B4's evidence.

## Multiple implementation steps

1. D13 (guarded consumption, re-checks, `source = local`) and D10's fragment link
   for the **existing email path**. These fix live defects and do not wait for the
   rest.
2. Migration (`issued_via`, `issued_by`), U37, and the D3 invalidations in U02, U04
   and U09. U10 gains `origin`.
3. The CLI operation, and D12's CLI MFA reset if the owner rules for it.
4. The web operation: confirm screen, D5/D6 refusals, D8 throttle, D10 response
   headers, i18n.
5. D7 notices and the account-page line.
6. Retire U06. Update the matrix, the manifest, `dangerous-operations.md`, the
   operator guide's handover procedure (D11), `docs/src/reference/audit-events.md`
   (which still lists `user.reset_password`), and `docs/threat-model.md`.

## Test plan

For every threat, a test that **fails when its control is removed**:
- **T2:** structural. The only production writers of a credential are user creation
  (U01), first setup, self-service change (U09) and token completion (U10). U06 is
  absent. The design review confirmed the list. First setup writes outside the seam
  (`crates/sui-id-core/src/setup.rs:202`) and is allowed by name.
- **T3/D5:** web issuance targeting an admin, and targeting self, is refused with
  no write.
- **T10:** a non-local target is refused on web and CLI; forgot-password for a
  non-local user sends nothing and returns the neutral response; completing a
  token whose user became non-local is rolled back.
- **T11/T12:** completing after the user is disabled is rolled back; two concurrent
  completions of one token give exactly one credential change.
- **T1/D6:**
  - an issuer without a second factor is refused;
  - a stale step-up redirects;
  - a missing reason or confirmation is refused.
- **T7/D3:**
  - a second issuance invalidates the first link;
  - the link is single use;
  - it expires at 30 minutes;
  - disabling the user invalidates it.
- **T6/D4:** after completion, an MFA-enrolled user must still pass the second
  factor.
- **T9/D10:**
  - the link appears in no log line, at any level (captured subscriber), with
    `log.access_log = true`;
  - the token appears in no request URI the server receives;
  - `GET /reset-password?token=…` is refused;
  - response headers are asserted;
  - no redirect carries it.
- **D8:** the sixth issuance within an hour is refused.
- **D9:**
  - one `user.recovery_link.issued` per issuance;
  - `origin` on completion;
  - injected append failure leaves no token.

## Security considerations

**What remains, stated plainly.**
- **An administrator can still take over a non-administrator account that has no
  second factor.** They issue a link to themselves and set a password. D7 and D9
  make that visible to the user and in the audit log, but they do not prevent
  it. That is inherent in any recovery an administrator performs; the remedy is
  requiring a second factor for users, which is outside this RFC.
- **The handover channel (T8) is procedural.**
- **Anyone with filesystem access to the key can use the CLI,** as they already
  can with every CLI subcommand and with the database itself.

- **One step-up authorizes up to D8's five issuances** within its five-minute
  window. Freshness is not bound to a single action; D8 bounds the effect.

**Why not let the administrator set the password.** Any administrator, or any
thief of an admin session, would learn a working credential for any account. The
audit log would show the user's later actions as the user's own, and the user
would learn nothing until they failed to sign in. The link removes the known
password (T2) and keeps the second factor in force (T6).

**Threat model.** `docs/threat-model.md` is the single home (RFC 098 rule 7).

## Findings recorded while scoping — not in scope

- The rotated client secret is placed in a redirect URL
  (`/admin/clients/{id}/edit?rotated_secret=…`). It lands in browser history and
  server access logs, and can leak through `Referer`. Fixed by the roadmap package
  [client-confirm-screens](../../roadmap/client-confirm-screens/README.md).

## Open questions

*All four ruled 2026-09-17 by `@nabbisen`, as recommended.*

1. **Expiry. Ruled: 30 minutes.** 30 minutes matches forgot-password and suits a real-time handover.
   Longer eases asynchronous handover but widens T4 and T7. Recommended: 30.
2. **Throttle. Ruled: five per administrator per hour, and five for the CLI.** Five per administrator per hour. Is that right for the expected
   deployment size?
3. **Refusing admin targets on the web. Ruled: refuse.** Recommended: refuse (T3). The cost is
   that an administrator who forgot their password needs someone with filesystem
   access. The owner confirms this trade-off, together with D12, as the design
   review advised.
4. **D12, CLI MFA reset for a sole administrator. Ruled: yes.**
