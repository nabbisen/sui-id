# Roadmap

This is a sketch of where sui-id is heading. Items are loose; nothing here is
a promise.

## Near term

(None outstanding — all near-term items shipped in v0.3.0. Next batch
draws from medium-term.)

## Medium term

The big-ticket auth features have all shipped (TOTP MFA, WebAuthn
passkeys, scope policy, post-logout URIs, signing key rotation, CSRF
tokens, edit page for clients, admin-initiated MFA reset). The
authentication and authorization surface is, for v0.x, broadly
complete; the natural next steps are operability and quality work
rather than new auth primitives.

- **Persistent email outbox + retry worker.** v0.22.0 sends mail
  inline with the request that triggered it; failures land in
  the audit log but the message itself is lost. For deployments
  where bounce rates are sensitive (password-reset to a flaky
  webmail provider) we'd add an `email_outbox` table with
  `queued / sending / sent / failed` states, a small in-process
  worker, and an exponential-backoff retry policy. Not needed
  at the current scale (sui-id sends ~1 mail per
  forgot-password / password-change), but a clear future
  enhancement once a deployment hits delivery problems.

- **i18n scope expansion (post-v0.23.0).** v0.23.0 ships the
  typed `sui-id-i18n` foundation — `Locale` enum, `Strings`
  struct with compile-time exhaustiveness on every translation —
  with two locales (Japanese, English) and the core admin/auth
  UI translated. The architecture is deliberately built so that
  expansion is incremental, not invasive. The threads we plan to
  pull on:
  - **More locales.** Add `Locale::Zh`, `Locale::Ko`, etc by
    declaring the variant and providing a `STRINGS_*` constant;
    the type system enforces every string is translated. No
    schema changes, no migration, no new dependencies. Specific
    languages will be prioritised by deployment demand.
  - **Date and number formatting.** Today timestamps render in a
    single ISO-ish format across locales; locale-aware date,
    time, and number formatting (`chrono::Locale`,
    `icu_decimal`-style) is deferred to a v2 pass when we have
    real-world feedback on which formats matter.
  - **Email template polish.** The forgot-password and password-
    change-notification templates are translated, but the prose
    is functional rather than polished. A native-speaker review
    pass for each shipped locale is on the list.
  - **Admin-facing audit-event labels.** Event *names* are
    stable English identifiers (operators query against them);
    the human-readable labels for the audit log UI are
    translated, but the long-form descriptions still need a
    pass per locale.
  - **Right-to-left support.** When a RTL locale (Arabic,
    Hebrew, etc) gets requested we'll need a CSS `[dir="rtl"]`
    pass on the design language. The `Locale::tag()` API is
    already the right anchor point for this.

- **HIBP scope expansion (post-v0.24.0).** v0.24.0 wires the
  Pwned Passwords check into the setup wizard's admin-creation
  step — the single password-set entry point that exists at
  install time. The remaining password-set entry points still
  bypass the check at this release; each is mechanical to add
  now that the `HibpClient` trait, the `enforce_hibp` policy
  function, and the `HibpMode` enum are in place. Specifically:
  - `me_security::password_change_post` (self-service password
    change) — should run the same check on the new password
    before the `credentials::upsert`.
  - `admin::users_password_reset` (admin-driven password reset
    for another user) — same check on the operator-supplied
    password.
  - `forgot_password::consume_and_reset_password` (token-based
    reset) — same check on the new password before the
    `credentials::upsert`.
  - Periodic re-check of stored passwords. Cannot work
    server-side (we don't store plaintext) but a "your last
    sign-in's password is now in a breach" notification on the
    next sign-in is feasible if we cache the SHA-1 prefix-only
    fingerprint at password-set time. The privacy story for
    that cache is non-trivial and is its own design discussion.
  - Admin settings UI. v0.24.0 has no UI for the `hibp_mode`
    column; an admin tab alongside Email is the next addition.

## Longer term, less certain

- **Federation.** Acting as an OIDC client to an upstream IdP, mapping
  the result onto a sui-id user.
- **Pluggable user backends.** A read-only LDAP shim, perhaps. The
  current storage layer assumes sui-id owns the user table.
- **Metrics.** A Prometheus endpoint behind admin auth.
- **Multi-tenancy.** Today every client and every user share one
  flat namespace. A tenant column threaded through the schema,
  with admins scoped to their tenant, would open up B2B-style
  deployments. The existing audit chain and cross-cutting
  policies (lockout, rate limits) all need to become
  tenant-aware first.
- **Outbound-facing-third-party scenarios.** sui-id today is
  designed for the "first-party" deployment model — every
  registered OIDC client is an application the same operator
  also runs. If we ever want to support the *other* posture —
  third-party developers registering clients against this IdP,
  end-users authorising those clients with their own data —
  several capabilities have to land together as a single
  coherent phase rather than as separate visual or schema
  changes:
    - **A consent screen.** The user must see "App X wants
      access to scopes Y and Z" and be able to approve, refine,
      or refuse. Either always-prompt or first-time-only with a
      stored grant; first-time-only is the usual choice (it
      matches the GitHub OAuth Apps shape) but needs a
      `user_consent` table, a scope-diff check on subsequent
      authorisations, and a UI on `/me/security` for the user
      to revoke a consent later.
    - **Dynamic client registration (RFC 7591).** Self-service
      registration for third-party developers. Currently on the
      "not on the roadmap" list specifically because it pulls
      sui-id toward this posture; if we adopt the posture, the
      ban is lifted.
    - **Per-client scope policy refinement.** Today
      `allowed_scopes` is a flat string. Sub-resource scopes
      (`read:profile` vs `write:profile`) and labelled, human-
      readable scope descriptions become important so the
      consent screen can show "This will let App X *read* your
      profile" rather than `read:profile`.
    - **Application identity.** A logo, a homepage URL, a
      privacy-policy URL per client — surfaced on the consent
      screen so the user has the context to decide.
    - **Refresh-token UX.** Long-lived refresh tokens against
      third-party clients need a per-user "Active applications"
      section on `/me/security` to revoke a specific client.
  Treat this whole bundle as one tagged phase ("third-party
  posture") rather than slicing it across releases. Splitting
  it produces a half-built consent UI that is worse than the
  current "no consent because all clients are first-party"
  story.

## Done

- Per-IP rate limiting on `/admin/login`, `/oauth2/token`, `/setup`.
- Background GC of expired authorization codes, sessions, refresh
  tokens, pending-MFA rows, and WebAuthn ceremonies.
- Audit logging of authentication outcomes (success/failure).
- `/healthz` endpoint suitable for liveness/readiness probes.
- crates.io publication metadata; binary distributable via
  `cargo install sui-id`.
- OpenID Connect RP-Initiated Logout 1.0 (`/oauth2/logout`).
- `server.trusted_proxies` opt-in for `X-Forwarded-For`-derived client IP.
- Annotated `sui-id.example.toml` configuration template.
- `sui-id backup` / `sui-id restore` subcommands with hot SQLite snapshot.
- `docs/threat-model.md` and a documentation index in the README.
- Signing key rotation UI with a JWKS grace window.
- CSRF tokens on every admin form (synchronizer-token + double-submit cookie).
- Per-client scope policy enforced at `/oauth2/authorize`.
- Per-client `post_logout_redirect_uris` (separate from `redirect_uris`).
- TOTP MFA (RFC 6238) with single-use recovery codes.
- Edit page for existing clients (name / redirect URIs / allowed scopes /
  post-logout redirect URIs).
- WebAuthn / passkey support (registration, authentication, multiple
  credentials per user, list / delete UI).
- Admin-initiated MFA reset (lifts every second factor for a user
  whose authenticator app, recovery codes, and passkeys have all
  been lost).
- `docs/deployment.md` — chronological install walkthrough from a
  fresh Linux server to a hardened production deployment.
- `cargo audit` CI integration — `.github/workflows/audit.yml` runs
  on push, on PR, and weekly. The companion `ci.yml` covers build,
  test, fmt, and clippy on every change.
- Token introspection (RFC 7662) and token revocation (RFC 7009).
  Confidential clients can introspect and revoke their own tokens
  via the standards-blessed endpoints; both are advertised through
  the OIDC discovery document.
- Structured logging — `X-Request-Id` propagation, per-request
  tracing spans, JSON-line output, a typed `SecurityEvent` API
  in `sui-id-core`, and an operator-facing event vocabulary
  documented in `operators.md` for SIEM integration.
- Server migration / secure backup. The `backup` and `restore`
  subcommands now write and read a `MANIFEST.json` (format
  version, schema version, sui-id version, creation timestamp,
  hostname, issuer); restore refuses too-new format or schema
  versions. `--encrypt` / `--decrypt` wrap the tarball in an
  XChaCha20-Poly1305 envelope keyed by an Argon2id derivation of
  an operator passphrase; the passphrase is read from stdin or
  `SUI_ID_BACKUP_PASSPHRASE`. A new `verify-backup` subcommand
  reads a file (decrypting if needed), parses the manifest, and
  runs SQLite `integrity_check` without writing anything.
- Property-based tests with `proptest`. Crypto seal/open round-
  trip, Argon2id round-trip, PKCE S256 derivation, the CIDR
  matcher (cross-checked against an independent brute-force
  reference), and the redirect-URI exact-match rule are all
  exercised with proptest. CONTRIBUTING.md documents the case-
  count convention and how to widen coverage with
  `PROPTEST_CASES=…` for releases or scheduled CI.
- `acr` and `amr` claims in ID tokens (OpenID Connect Core §2 +
  RFC 8176). The originating session's authentication factors are
  snapshotted at sign-in, propagated through auth codes and
  refresh-token rotations, and surfaced as numeric ISO 29115 LoA
  strings (`"1"` / `"2"` / `"3"`) plus an `amr` array. Refreshed
  ID tokens echo the original sign-in's claims rather than
  synthesising fresh ones. The `acr_values` request-side parameter
  is not yet honoured — relying parties filter on the issued
  claim.
- Account lockout. Per-account progressive backoff after
  consecutive password failures, with an operator-configurable
  cap (15min / 1h / 4h / 12h / 24h / 48h, default 24h) and an
  admin recovery command. The lockout check runs *before* Argon2id
  but a dummy verify still runs on the lockout branch so the
  timing of locked / unlocked / wrong-password / no-such-user is
  observably the same. A new `auth.login.locked` audit event
  distinguishes "wrong password" from "wrong password and now
  the account is locked" for SIEM consumption.
- Security strengthening pass (v0.17.0). Five reinforcements
  identified by an internal audit and shipped as one release:
  global security headers (CSP, HSTS, X-Frame-Options DENY,
  Permissions-Policy, …); CORS for the OIDC public endpoints
  (discovery / JWKS / userinfo as `*`, token-endpoint with
  origin allowlist from registered redirect_uris);
  `Cache-Control: no-store` on userinfo per OIDC Core §5.3.2;
  refresh-token theft detection (replay of a rotated token
  revokes the entire family with an `auth.refresh.theft_detected`
  audit event); and a hash chain on the audit log with a
  startup-time tail verification to detect DB-level tampering.
  Defense-in-depth removal of the dead `plain` branch in
  `verify_pkce`.
- Self-service security at `/me/security` (v0.18.0). Every
  signed-in user — admin or not — gets a per-account view of
  their active sessions, recent auth events touching their
  account (login success/failure/locked, MFA admin reset,
  refresh-token theft detection, …), and one-click revoke for
  individual sessions or "sign out everywhere else". Server-side
  ownership check refuses cross-account revoke without leaking
  whether the target id exists. New audit event
  `auth.sessions.bulk_revoke_self` records the bulk-revoke
  action. MFA enrollment itself stays on `/admin/profile`
  (unchanged); `/me/security` deep-links to it.
- Self-service password change at `/me/security/password`
  (v0.19.0). Current-password verify, policy check on the new
  one, and an opt-in "sign out everywhere else" sweep that
  revokes every other session and every active refresh token
  in one step. Current session stays alive so the user isn't
  booted out of the form. Rate limit against the shared `Login`
  bucket protects against grinding from a stolen cookie; no
  account lockout because the user is already authenticated
  by their session. Audit event `auth.password.changed_self`
  records sweep counts.
- Design language overhaul (v0.20.0). Lavender-Jade palette,
  light/dark themes (manual toggle + system follow), 4px
  spacing scale, 5-step typography scale, full component
  vocabulary (cards, badges, page headers, stats, table-wrap,
  flash, theme toggle). Core path of the UI rebuilt on the
  new tokens: login, admin nav, dashboard, users, clients.
  Other screens inherit colours and typography automatically
  and get their per-screen rework in v0.20.1. Multilingual font
  strategy locked in: system-ui only, no web fonts shipped, CJK
  via Unicode font fallback to OS-native fonts — distributed
  binary size unchanged.
- Per-screen design pass for the **non-core** pages (v0.20.1).
  setup / mfa-challenge / profile / mfa-setup / client-edit /
  audit / signing-keys / error / me/security /
  me/security/password rebuilt on the same component vocabulary
  as v0.20.0. Visual-only release: no handler logic, no schema,
  no auth changes. Japanese copy extended uniformly to the rest
  of the admin surface; operator-facing audit verbs and
  technical IDs stay in Latin. Three e2e tests followed the
  copy changes (one substring update each, plus a more robust
  secret-key extraction that anchors on "秘密鍵:" and walks past
  the inline-styled span).
- Dashboard sign-in sparkline (v0.20.2). Stacked-area chart of
  `auth.login.success` and `auth.login.failure` over the last
  24h / 7d / 30d, switchable from URL query. Inline SVG, no JS,
  per-bucket `<title>` tooltips. Migration 0011 adds a composite
  `audit_log(at, action)` index so the underlying time-window
  GROUP BY is a range scan even after the audit log grows into
  the millions. Range option default is 7 days; persistence is
  URL-only for now (localStorage layering is a small follow-up
  if we want it).
- Settings hub at `/admin/settings/*` (v0.20.3). Five-tab
  read-only overview surfacing the current effective
  configuration: Basic, Security, Authentication, Logs, Other.
  Each tab is its own route (URL is the source of truth for
  active tab; bookmark / refresh / back-button all just work).
  The Logs tab surfaces 24-hour counts of the four most
  operationally interesting audit actions and the audit-chain
  tail verifier status with a badge. Deep links into the
  existing detail pages (`/admin/users`, `/admin/clients`,
  `/admin/audit`) replace any need to re-implement controls.
- 3-step setup wizard (v0.20.4). The legacy single-page setup
  form splits into welcome → admin-creation → done across
  `/setup`, `/setup/admin`, and `/setup/done`. The admin form
  gains an optional email field (also surfaced on
  `/admin/users` for any new user) and password confirmation;
  migration 0012 adds the nullable `users.email` column with a
  partial unique index. The design memo's fourth screen
  ("encryption settings") is intentionally omitted — sui-id
  resolves the master key before HTTP is up, so there's no
  surface for a setup-time UI; the omission is documented in
  `docs/operators.md`. If a master-key rotation feature lands
  later it will be a CLI command, not a UI on a running
  process.
- Step-up authentication (v0.21.0). Sensitive admin and
  self-service actions gate on a fresh strong-factor proof:
  an MFA-enrolled user who hasn't completed a TOTP / recovery-
  code challenge in the last 5 minutes is bounced through
  `/me/security/step-up` before the action proceeds.
  Gated handlers: `me_security::revoke_all_others`,
  `admin::users_delete`, `admin::users_mfa_reset`,
  `admin::clients_delete`, `admin::signing_keys_rotate`,
  `admin::signing_keys_delete`. No-MFA users pass through
  transparently (a password re-prompt buys nothing against an
  attacker who has the cookie and password). Reversible
  operations (disable user, disable client) and operations
  whose primary credential gate is the user's own password
  (password change) are deliberately not gated, to avoid
  stacking proofs without security gain. WebAuthn-based
  step-up is a follow-up; the current release supports TOTP
  and recovery codes.
- WebAuthn step-up (v0.21.1). Closes the v0.21.0 gap for
  passkey-only accounts: a user whose only second factor is
  a registered passkey can now satisfy a step-up gate by
  completing a WebAuthn assertion ceremony. Migration 0013
  widens `webauthn_pending.kind` to allow `'step_up'`, so a
  step-up ceremony can never be misused as a login-MFA
  verification (and vice versa) even if a pending_id leaked
  across contexts. Two new core functions
  (`step_up::start_webauthn` / `finish_webauthn`) thinly wrap
  the existing assertion-flow code in `webauthn.rs` rather
  than duplicating it. Two new HTTP endpoints
  (`/me/security/step-up/webauthn/start` and `/finish`),
  a per-ceremony pending-id cookie, and a small
  `step-up-webauthn.js` driver complete the user-facing flow.
- Email features (v0.22.0). Forgot-password reset flow plus
  password-change notification emails. SMTP configuration
  lives in the database (singleton row, password sealed with
  the master key) so admins can change it without restarting
  and the settings page can offer a `Test Connection` button
  that runs a live EHLO/STARTTLS/AUTH dance against the
  configured relay. Reset tokens are 32 random bytes; only
  their SHA-256 hash is stored, single-use, 30-minute TTL.
  Sends are inline (the handler awaits the SMTP exchange);
  failures land in the audit log without affecting the
  user-enumeration-neutral response shape. Migrations 0014
  (`smtp_config`) and 0015 (`password_reset_tokens`) added.
  When SMTP is unconfigured / disabled, the email-related
  endpoints all 404 — the feature is opt-in.

  Built on `wasm-smtp` 0.9.3 (with `mail-builder` feature) and
  `wasm-smtp-tokio` 0.9. The integration was clean enough that
  the matching CHANGELOG entry contains a small list of
  upstream suggestions rather than complaints.
- Multilingual support v1 (v0.23.0). Typed `sui-id-i18n`
  foundation — `Locale` enum, `Strings` struct with compile-
  time exhaustiveness — backing a four-tier resolution chain
  (user.preferred_lang → cookie `sui_id_lang` →
  Accept-Language → `server_settings.default_lang` → Ja).
  Japanese and English ship at this release; the architecture
  enforces "every locale is a complete `Strings` constant" at
  compile time, so adding zh / ko / ... is purely additive
  with no schema changes. Migration 0016 adds
  `users.preferred_lang` (no CHECK constraint, so locale
  additions don't require a migration) and `server_settings`
  (singleton row pattern, modeled on `smtp_config`). The login
  page is fully translated at v0.23.0; the rest of the UI gets
  the same treatment in v0.23.x patches via the "i18n scope
  expansion" entry under Medium term — each per-page conversion
  is mechanical now that the plumbing (RequestLocale extractor,
  Strings struct, `<html lang>` on every page) is in place.
  Email locale resolution is decoupled from HTTP locale
  resolution: the email follows the recipient (their
  preferred_lang), the form follows the browser session.
- Pwned Passwords (HIBP) breach check (v0.24.0). Optional
  pre-acceptance check at the setup wizard's admin-creation
  step. Three operational modes (`'off' | 'warn' | 'block'`)
  stored in `server_settings.hibp_mode`, default `'warn'`.
  Uses the public Pwned Passwords API's k-anonymity scheme —
  sui-id sends only the first 5 characters of the SHA-1 hash,
  never the password itself, with `Add-Padding: true` to defend
  against traffic-analysis attacks. Fail-open: when the HIBP
  request fails (timeout, DNS, TLS, 5xx), the policy is to let
  the password through regardless of mode (including `block`),
  the audit trail records the failure, but a flaky external
  service is not allowed to lock an admin out of password
  operations. Built on `ureq` (synchronous) wrapped in
  `tokio::task::spawn_blocking` at the call site —
  the call rate is too low to justify async, and `ureq`'s
  rustls integration matches the `wasm-smtp-tokio` rustls
  already in our tree without pulling tokio's networking
  stack a second time. The remaining password-set entry points
  (self-service password change, admin reset, forgot-password
  redemption) and an admin-settings UI for the mode are
  scheduled in the "HIBP scope expansion" entry under Medium
  term.
- Idle session timeout and concurrent session cap (v0.25.0).
  Two adjacent self-hardening knobs over the existing UI-cookie
  session model. Idle timeout (`server_settings.idle_session_timeout_secs`)
  invalidates a session that has not been presented for the
  configured number of seconds; concurrent cap
  (`server_settings.max_concurrent_sessions`) limits a user to
  N simultaneous active sessions, evicting the oldest in FIFO
  order when a new login would exceed the cap. Both default to
  `0` (= disabled). The "most recent presentation" timestamp is
  written through a 60-second throttle so a busy session does
  not generate one DB write per HTTP request. Migration 0018
  adds `sessions.last_used_at`, the partial index
  `idx_sessions_user_active`, and the two settings columns; all
  three SessionRow construction sites pick up the new field.
  Idle-timeout enforcement lives inside `session::resolve` (best-
  effort revoke before refusing); concurrent-cap eviction lives
  in `enforce_concurrent_session_cap`, called from `login`,
  both `mfa::verify_pending` paths, and the WebAuthn finisher.
  The step-up freshness check (v0.21.x) is independent of the
  idle timeout — both must pass for sensitive actions; idle =
  "have you been here recently", step-up = "have you re-proven
  a strong factor recently". An admin tab on
  `/admin/settings/security` exposes both knobs with bilingual
  inline help spelling out the disabled-on-zero semantics and
  the FIFO behaviour.
- Master-key rotation CLI (v0.26.0). Re-seal every encrypted
  column under a new 32-byte XChaCha20-Poly1305 master key,
  with the old key file archived as `.bak.<timestamp>` beside
  the new one. Runs offline (server stopped, atomic SQLite
  transaction, no half-rotated state to recover from). Two
  new-key sources: `--generate-new-key` (CLI mints a fresh
  one) and `--new-key PATH` (operator-supplied). Default
  confirmation prompt; skip with `--yes` for non-interactive
  use. Six sealed columns are covered: signing keys, refresh
  tokens, TOTP secrets, TOTP recovery codes, WebAuthn
  passkeys, and the SMTP password. Hot/online rotation was
  rejected as the wrong cost-vs-complexity trade for an IdP.
- Threat-model refresh (v0.27.0). `docs/threat-model.md`
  rewritten from scratch to cover every defence shipped
  through v0.26.0. Five-part structure: Foundations, twelve
  Threat scenarios, eight Defensive properties, Detailed
  concerns (STRIDE breakdown, account-takeover attack-tree
  fragment, GDPR / SOC 2 / ISO 27001 / NIST 800-63B / OWASP
  ASVS V2 compliance hints, auditor FAQ), and Known
  limitations. Three reader profiles addressed explicitly
  (operators / developers, security auditors, enterprise
  adopters). Documentation-only release — no source changes,
  all tests continue to pass.
- Dev mode (v0.28.0). `sui-id --dev` brings up a fully
  working OIDC IdP in seconds against an in-memory SQLite
  database, with a pre-seeded admin / two test users / one
  OIDC test client. Aimed at developers building relying
  parties (RPs) who need a real IdP for local testing
  without clicking through the setup wizard each time.
  Hybrid seed model: hardcoded defaults +
  `--dev-admin-password` / `--dev-client-secret` flags +
  `--dev-seed PATH` TOML file (priority TOML > flags >
  defaults). Bind defaults to `127.0.0.1`; non-loopback
  bind via `--dev-bind` requires explicit `yes`
  confirmation on stdin. Cryptographic invariants
  preserved (PKCE S256-only, AAD binding, Argon2id
  parameters, `unsafe_code = forbid`, password-policy
  minimum length, `redirect_uri` exact match);
  operational knobs relaxed with operator-visible
  warnings (`cookie_secure off`, `hibp_mode off`,
  `lockout relaxed`).

## Explicitly **not** on the roadmap

- SAML.
- Implicit or hybrid OIDC flows.
- Dynamic client registration (RFC 7591) over the public internet,
  *while sui-id remains in the first-party deployment model*.
  See the "outbound-facing-third-party scenarios" entry under
  "Longer term, less certain" for what would have to land
  together to lift this restriction.
- A built-in clustering / multi-master mode.

The "not" list is not a list of bad ideas. It is a list of things that pull
sui-id in a direction it isn't trying to go.
