# UI/UX Contracts

> This document is the frozen cross-cutting UI/UX contract for sui-id's
> admin domain. It defines screen responsibilities, the dangerous-operation
> pattern, state copy conventions, and several domain-specific constraints.
> Every implementation RFC that touches `sui-id-web` or `sui-id-i18n` inherits
> these contracts. The contracts are not design suggestions — they are
> implementation requirements with a defined update process.
>
> Maintainers updating this document must increment the revision line and
> update cross-references in the relevant RFC files.
>
> **Revision:** v0.31.x (RFC 017).

---

## § 1. Screen relation map (five-stream isolation)

The product is decomposed into five streams, deliberately isolated from each
other:

```
[Uninitialised] ──setup token──▶ [First admin] ──auto──▶ [Login]
                                                           │
                       ┌───────────────────────────────────┼──────────────┐
                       ▼                                   ▼              ▼
              [Self-service]                       [Admin panel]   [OAuth/OIDC entry]
              /me/security                         /admin/...       /authorize ──▶ /token
              /admin/profile                       users/clients/
                                                   settings/audit
                       │                                   │
                       └───────────────────────────────────┘
                                       │
                       ┌───────────────▼──────────────────────────────────┐
                       │  Step-up + confirmation (dangerous operations)   │
                       │  TOTP / passkey, 5-minute freshness, fixed path  │
                       └──────────────────────────────────────────────────┘
```

**Contracts:**

- **Setup is one-shot.** After completion the route closes permanently.
  Re-opening is a maintenance CLI procedure, not a UI flow. (Governed by
  RFC 012.)
- **Admin panel and self-service do not share screens.** A user managing
  their own MFA uses `/admin/profile` or `/me/security`; an admin managing
  another user's MFA uses `/admin/users/<id>`. Same factor type; two screens
  because the subject differs.
- **OIDC RP entry is a sealed corridor.** External-app login goes through
  `/authorize` → login → consent → code and cannot leak into admin or
  self-service paths. The consent page has no "back to admin" link.
- **Dangerous operations sit on their own screen.** Step-up + a confirmation
  screen with an explicit impact summary + `return_to` fixed at the
  originating list page. No inline "Are you sure?" toggles on list rows.

---

## § 2. Screen responsibilities matrix

| Domain | URL pattern | Actor | Responsibility | Protection |
|---|---|---|---|---|
| Setup | `/setup/*` | uninitialised operator | one-shot bootstrap; closes on completion | setup token |
| Login | `/admin/login` | anonymous / RP-redirected | auth entry; branches to MFA / reset / authorize | CSRF |
| Self-service | `/me/security`, `/admin/profile` | authenticated user | own MFA / passkeys / sessions / language | session |
| Admin entry | `/admin` (dashboard) | admin | system status; dispatch to sub-domains | admin session |
| Users | `/admin/users/*` | admin | user CRUD; dangerous ops via step-up + confirm | step-up |
| Clients | `/admin/clients/*` | admin | OIDC client CRUD; redirect_uri constraints visible | step-up |
| Settings | `/admin/settings/*` | admin | safe form-driven edits; risky knobs isolated in Advanced tab | step-up |
| OIDC authorize | `/oauth2/authorize` | external-app user | post-login consent + code issue | PKCE |
| OIDC token | `/oauth2/token` | OAuth client | machine endpoint, no user-visible UI | client credentials |
| Audit | `/admin/audit` | admin | change history, read-only; secrets never displayed | admin session |

A new screen needs a row in this table — reviewed through the RFC
update process — before it merges.

---

## § 3. Dangerous-operation UI pattern

The following operations qualify as **dangerous**:

- Delete user (not recoverable via UI)
- Disable user (recoverable)
- Reset MFA (recoverable)
- Force logout (no-op if no sessions)
- Regenerate client secret (breaks existing consumers)
- Delete client (not recoverable via UI)

All six follow the same pattern:

```
[Trigger on list or detail page]
          │
          ▼
[Step-up challenge]          ← skipped if within 5-minute freshness window
          │
          ▼
[Confirmation screen]
  • Names the target (username, client name, count)
  • States the impact concretely: "This will revoke 3 active sessions
    for alice and 2 refresh tokens."
  • States reversibility: "Disable can be undone. Delete cannot."
  • Reversibility badge: green "Recoverable" or red "Not recoverable"
    — colour is never the only signal; badge text is always present
  • Primary button text is the explicit verb: "Disable alice",
    "Delete Web App 1" — never "OK" or "Yes"
  • Cancel returns to the originating list, no change made
          │
   confirm│
          ▼
[Mutation executed; audit row written; flash on returning list page]
```

**Per-operation details:**

- **Disable** — confirm screen reads: "alice will be unable to sign in
  until re-enabled. Re-enabling is available from the user list." The
  disable reason field (optional) is written to `audit.note`.
- **Delete** — soft-delete; confirm screen reads: "alice will no longer
  appear in user lists. Their audit history is preserved. This cannot be
  undone from the admin panel." Hard-delete is a CLI operation
  (`sui-id admin delete-user --hard`) with a documentation link on the
  confirm screen.
- **Reset MFA** — confirm screen reads: "alice will need to re-enrol
  their TOTP authenticator and any passkeys at next sign-in."
- **Force logout** — confirm screen displays the count: "3 active sessions
  and 2 refresh tokens will be revoked immediately." If count is zero, the
  screen still renders and the action creates an audit row.
- **Regenerate client secret** — confirm screen reads: "The current
  secret will stop working immediately. The new secret is displayed once
  and cannot be retrieved afterwards." The new secret renders on the
  success path of the confirmation screen.
- **Delete client** — confirm screen displays scope: "Web App 1 will be
  removed. N active sessions issued to this client will be revoked."

---

## § 4. State copy contract

Every screen declares copy for these five states in both `ja` and `en`
before implementation begins. The copy lives in `sui-id-i18n::Strings`
(typed keys; RFC 002 enforces exhaustive matching). A screen is not
implementation-complete until all five states have copy.

| State | When it appears | Tone |
|---|---|---|
| `loading` | data fetch in flight | neutral, brief: "読み込み中…" / "Loading…" |
| `empty` | data fetched, none returned | actionable: "ユーザーは登録されていません。最初のユーザーを作成してください。" / "No users yet. Create the first user." |
| `success` | mutation completed | confirms what changed, names the target: "alice を停止しました。3 件のセッションを失効。" / "Disabled alice. 3 sessions revoked." |
| `error` | mutation failed | brief, no internal detail, references the request ID: "保存できませんでした。詳細はリクエスト ID xxx で確認できます。" / "Save failed. Reference request ID xxx for details." |
| `disabled` | action unavailable in current state | states the precondition: "MFA を解除するには step-up 認証が必要です。" / "Removing MFA requires step-up authentication." |

---

## § 5. Admin dashboard information policy

The dashboard is a **dispatcher, not a workplace**. It surfaces operational
status and routes operators to the appropriate sub-domain. It does not
perform mutations.

### Always shown

- Initialisation status.
- Public-endpoint reachability flags: `/openid-configuration` returns 200,
  `/jwks.json` returns 200, DB open, SMTP configured or not.
- Counts: registered users, registered clients, active sessions.
- Recent important events (last 5–10 from audit log, admin-domain events
  only).
- Operator action prompts — shown only when the condition is true; absent
  when resolved:
  - "SMTP not configured — forgot-password email is disabled."
  - "Master key was generated by sui-id. Back up `sui-id.key` now."
  - "HIBP mode is `off`. Password breach checking is disabled."
  - "`cookie_secure` is `false`. Set to `true` in production."
- Audit hash-chain verification status.

### Never shown on the dashboard

- Any secret value (master key bytes, client secrets, tokens of any kind).
- Per-user analytics (login frequency, geographic distribution).
- Buttons for dangerous operations. The dashboard *links* to sub-domain
  lists; dangerous actions live on the corresponding detail screens.
- Full TOML configuration text.

---

## § 6. Settings tab structure

Six tabs, fixed order:

| Tab | Content |
|---|---|
| Basic | Service name, base URL (issuer), default language |
| Security | `cookie_secure`, idle-session timeout, max concurrent sessions per user |
| Authentication | `hibp_mode` (off / warn / block), lockout policy, MFA requirement policy |
| Email | SMTP host / port / TLS mode / credentials, sender name, base URL for links |
| Logs | Audit log retention guidance, tracing log filter (read-only display) |
| Advanced | Master-key fingerprint, signing-key rotation triggers, other risky settings |

**Advanced tab isolation.** Master key management, low-level TOML editing,
and any future dangerous configuration knobs live exclusively in the Advanced
tab. Other tabs contain only safe form-driven edits. The Advanced tab
renders these as read-only fields with explicit links to CLI procedures
(`sui-id admin rotate-key`, backup/restore documentation); it does not
offer in-page mutation buttons for master-key operations.

The spec uses "Other" for the sixth tab; this contract renames it "Advanced"
to communicate the isolation intent. The `sui-id-i18n` key is
`settings_tab_advanced`.

---

## § 7. Client management UI: constraints made visible

Every screen that creates or edits a client surfaces these constraints
inline (not in a tooltip; not in linked documentation):

- **One flow: Authorization Code + PKCE (S256) only.** There is no flow
  selector.
- **`redirect_uri` is exact-match.** Form help text: "Each URI is matched
  exactly. Wildcards and prefix matching are not supported." Redirect URIs
  display as a vertical list, never comma-joined.
- **Confidential vs public is final at create time.** Help text: "Public
  clients (browser SPAs, mobile apps) use PKCE without a secret.
  Confidential clients (server-side apps) use a client secret in addition
  to PKCE. This cannot be changed after creation."
- **Client secret display: once, on create or regenerate.** Confirmation
  message: "Save this secret now. It will not be shown again. If lost, it
  must be regenerated, which will invalidate any application currently
  using the current secret."
- **`post_logout_redirect_uris` is a separate field.** Not merged with
  `redirect_uris`. Same exact-match rule.
- **`allowed_scopes` is a separate field** with the catalog of known
  scopes (`openid`, `profile`, `email`, `offline_access`) displayed as
  helper text. See also [the operator guide](src/guides/operators.md),
  "User–client relationship and the single-realm model".

The **consent screen** (RFC 008) inherits these contracts: scope text and
client name must be readable by screen readers; consent buttons say
"Allow" and "Refuse", not "Yes/No"; optional scopes default to unchecked.

---

## § 8. Audit log display rules

The audit log is a **forensic surface**:

- **Event names are stable identifiers**, dot-separated lowercase:
  `auth.login.success`, `admin.users.disable`. They are never translated.
  The translated human-readable label (RFC 002 § D) renders alongside.
- **Secret values are never displayed** — not in rows, details, or
  exports. Inherits RFC 016's redaction invariant.
- **Failures show the result code, not the reason.** `result = "failure"`
  plus an opaque `failure_kind` enum value. No free-text reason field.
- **Hash-chain status is surfaced** as a persistent banner at the top of
  the screen: "Audit chain verified through row N (last checked HH:MM)."
  A mismatch renders in red and links to the operator runbook.
- **Filters are minimal**: time range, event-name prefix, actor/target
  user. No free-text search across the `note` field.
- **Export escape valve**: a "Copy row ID" button on each row and a CSV
  export of filtered rows. No JSON export, no built-in syslog forwarder.

---

## § 9. Dev mode UI separation

- **Startup banner** (stdout/stderr) shows admin / alice / bob credentials
  and the auto-assigned client ID under a clear "DEV MODE" header.
- **No dev credentials in browser screens.** The login page never
  pre-fills or hints the admin password. Credentials come from the
  terminal.
- **Persistent dev banner in the browser.** Every page rendered while
  sui-id is running with `--dev` shows a yellow ribbon at the top:
  > "DEV MODE — not for production. cookie_secure=false, HIBP off,
  > lockout disabled."
  Same wording in both `ja` and `en`.
- **Settings shows dev relaxations as warnings.** The Security tab shows
  `cookie_secure = false` with a warning icon and the text "Dev default.
  Production must set this to true." The field is read-only with an
  explanation; it is not silently disabled.
- **Non-loopback bind prompts interactively** (CLI check); the browser dev
  banner says "BIND: 0.0.0.0 — network-reachable" in red when applicable.
- **The setup wizard is unreachable in dev mode.** `/setup` redirects to
  `/admin` because the dev seed is treated as initialised.

---

## § 10. Accessibility implementation contract

Every screen shipped in `sui-id-web` must satisfy:

- Every form control has a visible `<label>` element.
- Every input has `aria-describedby` pointing at its help text.
- Tab order matches visual top-to-bottom, left-to-right reading order.
  No `tabindex` value greater than 0.
- `:focus-visible` shows a 2 px outline in the accent colour.
  This is non-negotiable — PRs that remove it are rejected as bugs.
- Status colours always pair with text or icon ("✓ Verified", "✗ Failed"),
  never colour alone.
- Modal dialogs trap focus while open and restore focus to the trigger on
  close.
- Keyboard activation works on every interactive element: Enter activates
  links and buttons, Space activates buttons, arrow keys navigate radio
  groups.
- Error messages render inside `role="alert"` regions so screen readers
  announce them without requiring focus movement.

---

## § 11. Text selection contrast

The `::selection` pseudo-element must meet WCAG 2.1 SC 1.4.3 contrast
requirements (4.5:1 for normal text, 3:1 for large text) in both light
and dark modes. The token values are defined in RFC 023
(visual design system). This is listed here because the selection-contrast
requirement is accessibility-critical and must be verified on every screen
that displays credential values (Client ID, secret, user UUID).

---

## Appendix: cross-references to dependent RFCs

| RFC | Which § applies |
|---|---|
| RFC 002 — i18n expansion | § 4 state copy contract (keys per screen) |
| RFC 003 — HIBP scope | § 6 settings tab structure (Authentication tab placement) |
| RFC 008 — third-party posture | § 7 client management (consent screen) |
| RFC 010 — forgot-password revoke | § 4 post-reset success copy |
| RFC 011 — WebAuthn transport enforcement | § 4 state copy for transport errors |
| RFC 012 — setup wizard reconciliation | § 1 one-shot setup, § 6 post-setup settings landing |
| RFC 016 — server logging | § 8 audit display (redaction invariant) |
| RFC 023 — visual design system | § 10, § 11 focus ring, selection contrast |
