# RFC 017 — UI/UX design contracts: screen responsibilities, danger patterns, state copy

**Status.** Proposed
**Priority.** Medium. Recommended ahead of any new admin-UI
implementation work — specifically RFC 002 (i18n expansion into
admin domain) and any further admin-domain UI work — so those
features inherit a fixed contract instead of inventing one each.
**Tracks.** Maintainer-supplied UI/UX deliverables (one-page
overview + 17-page detailed deck, v0.29.x).
**Touches.** No code changes in this RFC. Output is a new
`docs/ui-ux-contracts.md` referenced from existing RFCs and
from `docs/operators.md`. Subsequent implementation work in
`sui-id-web` and `sui-id-i18n` follows the contracts established
here.

## Summary

The two UI/UX deliverables describe screen-level responsibilities,
inter-screen relationships, and copy/state conventions that
currently live as design intent rather than as a written contract
the implementer can reference. Several individual RFCs (010
through 016) carry pieces of this — RFC 010's post-reset screen
copy, RFC 011's WebAuthn-transport user-facing message, RFC 012's
setup-flow shape, RFC 003's settings UI placement — but none of
them establishes the *cross-cutting* conventions that make those
pieces consistent with each other.

This RFC is that cross-cutting layer. It does not introduce new
features. It freezes contracts so that implementers picking up
RFC 002 and any future admin-domain UI work produce output that
doesn't drift from the rest of the surface.

## Why this RFC, not eight smaller ones

Each sub-contract below — screen responsibilities, dangerous-
operation pattern, state copy, dashboard policy, settings tabs,
audit display, dev-mode separation, client-management
constraints — could be its own document. They are bundled here
because:

- The PDF deliverables make a single coherent argument
  ("画面を増やすより、責務を分ける。説明を増やすより、次の安全な
  行動を明示する"). Splitting it into pieces obscures the
  argument.
- Each piece references the others. The dangerous-operation
  pattern relies on the state-copy contract; the settings tab
  structure relies on the dashboard policy; the audit display
  rules rely on the redaction invariant from RFC 016. The
  cross-references are tighter than the boundaries between
  topics.
- Implementers reading these in isolation would re-derive the
  same overall shape from each one. Bundling avoids that.

The deliverables explicitly distinguish "已实现 / 要判断" — items
that are settled vs items requiring maintainer decision. This RFC
inherits that distinction: where a sub-contract has a settled
shape it freezes the shape; where it has open questions it
defers to the relevant existing RFC (notably RFC 012 for setup,
RFC 003 for HIBP admin UI).

## Sub-contracts

### § 1. Screen relation map (5-stream isolation)

The product is decomposed into five streams, deliberately
isolated:

```
[Uninitialised] ──setup token──▶ [First admin] ──auto──▶ [Login]
                                                           │
                                ┌──────────────────────────┼──────────────┐
                                ▼                          ▼              ▼
                       [Self-service]              [Admin panel]    [OAuth/OIDC RP entry]
                       /me/security                /admin/...        /authorize ──▶ /token
                       /admin/profile              users/clients/
                                                   settings/audit
                                │                          │
                                ▼                          ▼
                       [Step-up + confirmation for dangerous operations]
                       (TOTP / passkey, 5-minute freshness, return_to fixed)
```

Contracts:

- **Setup is a one-shot path.** After completion, the route
  closes. Re-opening is a maintenance procedure, not a UI
  flow. (Sequenced with RFC 012.)
- **Admin panel and self-service do not share screens.** A
  user managing their own MFA does it under `/admin/profile`
  or `/me/security`; an admin managing another user's MFA
  does it under `/admin/users/<id>`. Same factor type; two
  different screens because the operator is different.
- **OIDC RP entry is a sealed corridor.** External-app login
  goes through `/authorize` → login → consent → code, and
  cannot leak into admin or self-service paths. The "back
  to admin" link is absent from the consent page.
- **Dangerous operations sit on their own screen.** Step-up
  + a confirmation screen with explicit impact summary +
  `return_to` fixed at the originating list page. No inline
  "Are you sure?" toggles on lists.

### § 2. Screen responsibilities matrix

| Domain | URL | User | Responsibility | Protection |
|---|---|---|---|---|
| Setup | `/setup` | uninitialised operator | one-shot bootstrap, closes on completion | setup token |
| Login | `/admin/login` | anonymous / RP-redirected | auth entry; branches to MFA / reset / authorize | CSRF |
| Self-service | `/me/security`, `/admin/profile` | authenticated user | own MFA / passkeys / sessions / language | session |
| Admin entry | `/admin` | admin | system status, dispatch to sub-domains | admin |
| Users | `/admin/users` | admin | user management; dangerous ops via step-up + confirm | step-up |
| Clients | `/admin/clients` | admin | OIDC client management; redirect_uri exact-match displayed | step-up |
| Settings | `/admin/settings/*` | admin | safe form-driven edits; risky knobs isolated | step-up |
| OIDC authorize | `/oauth2/authorize` | external-app user | post-login consent + code issue | PKCE |
| OIDC token | `/oauth2/token` | OAuth client | machine endpoint, no UI | client cred |
| Audit | `/admin/audit` | admin | change history, secrets never displayed | admin |

Implementer reading this matrix must be able to answer "what
does this screen do, who uses it, and what stops it from being
abused" in one row. If a new screen needs a row, it goes
through this RFC's update process before merging.

### § 3. Dangerous-operation UI pattern

Four operations across the admin surface qualify as dangerous:
**Delete user**, **Disable user**, **Reset MFA**, **Force
logout** — and, on clients, **regenerate secret** and **delete
client**. They share a fixed pattern.

```
[Trigger from list/detail]
          │
          ▼
[Step-up challenge]   ← only if not within 5-minute freshness
          │
          ▼
[Confirmation screen]
   ─ identifies the target (username, client name)
   ─ states the impact in concrete terms
        ("This will sign out all 3 active sessions for alice.")
   ─ states reversibility
        ("Disable can be undone. Delete cannot.")
   ─ requires explicit "Confirm <verb>" button text — not
     "OK", not "Yes"
   ─ has a Cancel button that returns to the originating list
          │
   confirm│
          ▼
[Audit row written; success flash on returning list page]
```

Per-operation contracts:

- **Disable** — recoverable. Confirm screen states "User can
  be re-enabled by an admin." Optional reason field that goes
  into the audit row's `note`.
- **Delete** — soft-delete by default; the confirm screen
  states the user "will no longer appear in lists but their
  audit history is preserved." Hard-delete is a CLI operation
  (`sui-id admin delete-user --hard`), referenced from this
  screen as a documentation link, never as an in-page button.
- **Reset MFA** — confirm screen explains "the user will need
  to re-enrol TOTP and any passkeys at next sign-in." Audit
  row records the resetting admin and the target user.
- **Force logout** — confirm screen displays the count of
  sessions and refresh tokens that will be revoked. If the
  count is zero, the screen says so and the action is no-op
  (still creates an audit row).
- **Client secret regenerate** — confirm screen warns "the
  current secret will stop working immediately. The new
  secret will be displayed once and not retrievable
  afterwards." Same screen renders the new secret on the
  success path.
- **Client delete** — confirm screen displays affected scope:
  number of active sessions issued to this client, refresh
  tokens, etc.

The confirmation screen always shows reversibility status as
a coloured badge (green = recoverable, red = not). Colour is
*not* the only signal — the badge text says "Recoverable" or
"Not recoverable" verbatim.

### § 4. State copy contract

Every screen declares its copy for these states up front, in
both Ja and En, before implementation begins:

| State | When | Tone |
|---|---|---|
| `loading` | data-fetch in flight | neutral, brief: "読み込み中…" / "Loading…" |
| `empty` | data fetched, none returned | actionable: "ユーザーは登録されていません。最初のユーザーを作成してください。" / "No users yet. Create the first user." |
| `success` | mutation succeeded | confirms what changed: "alice を停止しました。3 件のセッションを失効済み。" / "Disabled alice. 3 sessions revoked." |
| `error` | mutation failed | brief, internal-detail-free, references the request-id: "保存できませんでした。詳細はリクエスト ID xxx で確認できます。" / "Save failed. Reference request ID xxx for details." |
| `disabled` | action unavailable in current state | states the precondition: "MFA を解除するには step-up 認証が必要です。" / "Removing MFA requires step-up authentication." |

The copy lives in `sui-id-i18n::Strings`. RFC 002 already
defines the exhaustive-match enforcement; this RFC adds the
*completeness rule per screen*: a screen is not
implementation-complete until all five states have copy.

### § 5. Admin dashboard information policy

The admin dashboard is a **dispatcher, not a workplace**.
Concrete contracts:

#### Always shown

- Initialisation status (initialised / not).
- Public-endpoint reachability flags: `/.well-known/openid-
  configuration` returns 200, `/jwks.json` returns 200, DB
  open, SMTP configured/not.
- Counts: registered users, registered clients, active
  sessions.
- Recent important events (last 5–10 from audit log,
  filtered to admin-domain events).
- Operator action prompts: "SMTP not configured — forgot-
  password disabled", "Master key generated by sui-id —
  back this file up", "HIBP mode is `off`", "Cookie secure
  flag is off — production deployments should set it true".
- Hash-chain verification status.

#### Never shown

- Any secret value (master key fingerprint is acceptable;
  the bytes themselves never).
- Any token value (access, refresh, ID, reset, CSRF).
- Inline buttons for dangerous operations. The dashboard
  *links* to user/client lists; dangerous actions live on
  the corresponding detail screens.
- Per-user analytics (login frequency, geographic
  distribution, time-of-day patterns). Out of scope.
- TOML config full text. The Settings sub-screens render
  individual form fields.

The "next operator action" prompts are not noisy. They
appear only when the underlying condition is true; they
disappear when resolved. A clean deployment shows zero
prompts.

### § 6. Settings UI tab structure

Six tabs, fixed:

| Tab | Houses |
|---|---|
| Basic | service name, base URL, default language |
| Security | `cookie_secure`, idle session timeout, max concurrent sessions |
| Authentication | `hibp_mode` (off/warn/block), lockout policy, MFA policy |
| Email | SMTP host/port/TLS mode/auth, sender name, base URL for links |
| Logs | audit log retention guidance, tracing log filter (read-only display of current value) |
| Advanced | master-key fingerprint, signing-key generation count, dangerous knobs |

The **Advanced tab** is the isolation boundary for risky
settings. Master key, signing-key rotation triggers, low-level
TOML editing — none of these are inline form fields anywhere
else. The Advanced tab itself contains read-only displays plus
explicit links to CLI procedures (`sui-id admin rotate-key`,
backup/restore documentation).

The spec text says "Basic / Security / Authentication / Logs /
Email / Other"; this RFC renames "Other" to "Advanced" because
the rename communicates the isolation intent more clearly. Spec
amendment is a one-line change in `docs/operators.md`.

(RFC 003 places `hibp_mode` UI under the Authentication tab.
This RFC ratifies that placement. RFC 002's i18n expansion
covers the strings for all six tabs.)

### § 7. Client management UI: constraints made visible

Every screen that creates or edits a client must surface these
constraints inline (not in a tooltip, not in linked docs):

- **Authorization Code + PKCE (S256) only.** No flow
  selector — there is one flow.
- **`redirect_uri` is exact-match.** When entering URIs, the
  form shows: "Each URI must match exactly. Wildcards and
  prefix matching are not supported." Listed redirect URIs
  display as a vertical list, never comma-joined.
- **Confidential vs public.** Choice is final at create time.
  Help text: "Public clients (browser SPAs, mobile apps) use
  PKCE without a secret. Confidential clients (server-side
  apps) use a secret in addition to PKCE."
- **Client secret display, on create or regenerate.** The
  secret is shown once. The confirmation message is unambiguous:
  "Save this secret now. It will not be shown again. If
  lost, it can be regenerated, which will break any
  application currently using the old secret."
- **`post_logout_redirect_uris` is a separate field.** Not
  combined with `redirect_uris`. Same exact-match rule.
- **`allowed_scopes` is a separate field, displayed
  alongside the catalog of supported scopes.**

The consent screen (when RFC 008's third-party-posture bundle
lands) inherits these contracts: scope text and client name
must be readable by screen readers; the buttons say "Allow"
and "Refuse", not "Yes/No"; optional scopes default to
unchecked.

### § 8. Audit log display rules

The audit log screen is a **forensic surface**, not a
debugging surface. Contracts:

- **Event names are stable identifiers.** Dot-separated,
  lowercase: `auth.login.success`, `admin.users.disable`,
  `auth.password.reset_via_email`. They are *not*
  translated. The translated *human-readable label* (RFC
  002 § D) renders alongside.
- **Secret values are never displayed.** Not in the row,
  not in the details modal, not in any export. This
  invariant inherits from RFC 016's redaction list.
- **Failures show the result code, not the reason.**
  `result="failure"` plus an opaque `failure_kind` enum
  ("wrong_password", "csrf_mismatch", "rate_limited",
  "lockout_active", …). No free-text reason field.
- **Hash-chain status is surfaced.** A persistent banner at
  the top of the screen shows "Audit chain verified through
  row N (last checked HH:MM)". A mismatch shows red and
  links to the operator runbook (in `docs/operators.md`).
- **Filters are minimal.** Time range, event-name prefix,
  actor/target user. No free-text search across the `note`
  field — that's where event-specific context lives, but
  it's audit metadata, not a search corpus.
- **Export is the operator's escape valve.** A "copy row
  ID" button on each row, plus a CSV export of the
  filtered rows, is sufficient. No JSON export, no
  syslog forwarder built-in (out of scope).

### § 9. Dev mode UI separation

`--dev` provides convenience, but the *visual contract*
keeps it from leaking into production-mental-model:

- **Startup banner shows credentials.** stdout/stderr at
  process start displays admin / alice / bob credentials
  and the auto-assigned client ID, with a clear "DEV MODE"
  header. Operator can copy from there.
- **No dev credentials in browser screens.** The login
  page does not display the admin password as a hint, even
  in dev. Operator gets the credential from the terminal,
  not the UI.
- **Persistent dev banner in the browser.** Every page in
  a dev-mode-running sui-id renders a yellow ribbon at the
  top: "DEV MODE — not for production. cookie_secure=false,
  HIBP off, lockout disabled." Same wording in Ja / En.
- **Settings shows dev relaxations as warnings.** The
  Security tab in dev mode displays `cookie_secure = false`
  with a warning icon and the text "Dev default. Production
  must set this to true." The settings tab does not silently
  disable form controls; the field is read-only with an
  explanation.
- **Non-loopback bind requires interactive confirmation.**
  Spec sec. 11.13 already requires this; the UI counterpart
  is that the dev banner explicitly says "BIND: 0.0.0.0 —
  network-reachable" in red when applicable.
- **The setup wizard is not reachable in dev mode.** Even
  if an operator manually navigates to `/setup`, the route
  redirects to `/admin` because the dev seed counts as
  initialised. This prevents the "production setup wizard
  in dev" mental-model leak.

### § 10. Accessibility implementation contract

The spec already establishes ABDD. This RFC fixes the
per-screen checklist:

- Every form control has a visible `<label>`.
- Every input has an `aria-describedby` for help text.
- Tab order matches visual top-to-bottom, left-to-right
  order. No `tabindex > 0` anywhere.
- `:focus-visible` shows a 2px outline in the accent
  colour. This is non-negotiable — colour selectors that
  remove it are bugs.
- Status colours always pair with text or icon ("✓
  Verified", "✗ Failed"), never colour-only.
- Modal dialogs trap focus and restore it on close.
- Keyboard activation works on every interactive element
  (Enter on links and buttons, Space on buttons, arrow
  keys on radio groups).
- Error messages render in `role="alert"` regions.

## Cross-references to existing RFCs

Where this RFC's contracts intersect existing in-flight RFCs,
the contracts are **constraints inherited by the implementer
of those RFCs**, not separate work items here:

- **RFC 010 (forgot-password revoke)** — § 4 state copy
  contract: the post-reset success page text must declare
  "All other sessions for this account have been signed out."
  in the spec'd state-copy shape.
- **RFC 011 (WebAuthn transport)** — § 4 state copy: when
  the server-side check rejects an HTTP origin at startup,
  the operator-visible error is a state-copy "error" string,
  not a stack trace. The browser-side message ("Passkeys
  require HTTPS") follows the same shape.
- **RFC 012 (setup wizard reconciliation)** — § 1 screen
  relation map locks the "setup is one-shot, closes on
  completion" contract regardless of whether Position A,
  B, or C is chosen. § 6 settings tab structure determines
  where deferred-from-setup choices land if Position A or
  C is picked.
- **RFC 002 (i18n expansion)** — § 4 state copy contract
  defines the *what*; RFC 002 defines the *how* (typed
  Strings keys, exhaustive match). The RFC 002
  implementer reads § 4 to know which keys must exist
  per screen.
- **RFC 003 (HIBP scope)** — § 6 settings tab structure
  ratifies the placement of `hibp_mode` UI under the
  Authentication tab. Wording follows § 4.
- **RFC 008 (third-party posture)** — § 7 client
  management constraints inform the consent screen's
  shape. § 3 dangerous-operation pattern applies to the
  consent revocation flow.
- **RFC 016 (server logging)** — § 8 audit display
  inherits the redaction invariant; no audit row contains
  values listed in RFC 016's "never logged" set.

## Multiple implementation steps

This RFC's deliverable is a single document
(`docs/ui-ux-contracts.md`) that other RFCs reference. It
lands in one piece. Subsequent enforcement is via the
existing RFC implementations:

1. Land this RFC's document, with cross-references to RFCs
   002, 003, 008, 010, 011, 012, 016.
2. Update those RFCs' "Touches" / "Open questions" sections
   to point back at the relevant § here. (Edits to existing
   RFCs are small and acceptable.)
3. Implementers picking up RFC 002 / future admin-domain
   work consult the document as the contract.

No code changes. No tests. No release-blocking checklist.

## Tests

Not applicable as a code-test artifact. The contracts are
enforced through the test suites of the *referencing* RFCs:

- RFC 002's translation-completeness test ensures § 4 state
  copy keys exist for each screen.
- RFC 016's redaction tests ensure § 8's audit display
  never leaks secrets.
- RFC 010's e2e regression covers the § 4 post-reset
  success copy.

A reasonable follow-up — outside this RFC's scope — would
be a `cargo xtask` script that walks every Leptos component
in `sui-id-web` and asserts each one declares all five
state-copy variants. That's a tooling discussion, not a
contract-document discussion. Logged as future work.

## Security considerations

The contracts themselves are security-relevant:

- **§ 3 dangerous-operation pattern** prevents accidental
  destructive admin actions by separating intent (the
  trigger) from confirmation (the explicit verb).
- **§ 5 dashboard information policy** limits the
  dashboard's information density to operationally
  necessary data, reducing accidental disclosure if a
  shoulder-surfer or a screenshot leaks the page.
- **§ 7 client management constraints** make the
  one-time secret display unambiguous, reducing the
  chance of an admin saving the wrong value.
- **§ 8 audit display rules** preserve the hash chain's
  forensic value by refusing to surface anything that
  could be misread as authoritative if doctored.
- **§ 9 dev mode separation** prevents dev-mode
  configuration from being mistakenly carried into
  production through visual familiarity.

No new attack surface is introduced by this RFC; the
document codifies posture that already exists implicitly,
making it explicit and reviewable.

## Resolved decisions (formerly Open questions)

The following decisions were resolved with the maintainer
during v0.29.4 review:

- **Contract document path.** Lands at
  `docs/ui-ux-contracts.md`. (Not under
  `docs/architecture/` — sui-id has no `architecture/`
  subdirectory and creating one for a single document
  would be premature.)
- **Source-PDF archival in the repo.** **No.** The PDF
  deliverables are still under iterative refinement and
  may continue to evolve as the design language develops.
  Archiving them in the repo would freeze a specific
  revision and create maintenance friction. Operators
  needing the visual reference go to the maintainer's
  out-of-band design materials. Within the repo,
  `docs/ui-ux-contracts.md` is the authoritative
  written contract; the PDFs informed it but are not the
  source of truth going forward.
- **Tab naming: "Advanced" vs spec's "Other".**
  Renamed to "Advanced" in the contract document and a
  one-paragraph spec amendment to sec. 11.7 follows in
  the same change.
- **Typed-confirmation field for delete operations.**
  Not adopted in v1. Step-up + an explicit verb button
  (per § 3) is the contract. Revisit only if real
  operator feedback surfaces accidental deletions.

## Open questions

None at time of acceptance. If subsequent admin-UI work
surfaces an unresolved contract gap, it goes through this
RFC's update process rather than ad-hoc per-PR judgement.
