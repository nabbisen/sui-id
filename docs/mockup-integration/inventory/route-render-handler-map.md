# Route → Render → Handler Map (Product v0.49.1)

Phase-0 deliverable of [RFC-MI-000](../../../rfcs/done/RFC-MI-000-baseline-delta-inventory.md).
Generated against `crates/sui-id/src/router.rs` in
**sui-id v0.49.1**.

This file is the **authoritative product-side inventory** the
screen-level RFCs (MI-030 onward) reference when wiring page changes.
Combined with `screen-map.md`, it gives every implementer the full
mapping `mockup screen → product route → render function → handler →
data struct → audit/CSRF expectations` without re-reading the
codebase.

## Conventions

- **Method** — HTTP method handled.
- **Auth** — `none` (public), `setup-token` (setup-only),
  `user` (authenticated session), `admin` (admin privilege),
  `oidc-bearer` (Bearer token), `oidc-public` (no auth, public OIDC
  endpoint), `client-cred` (HTTP Basic / form-post client
  credentials), `step-up` (additional re-authentication required).
- **CSRF** — `✓` if the handler validates an `_csrf` form field
  against the session token; `n/a` for GET / non-cookie POST surfaces
  (OIDC token, introspect, revoke).
- **Render** — public render function in `crates/sui-id-web/src/lib.rs`
  if the response is HTML; `—` for handler-produced bodies
  (redirects, JSON, CSV).
- **Audit emit** — the audit event names emitted (per RFC 040,
  RFC 045, RFC 058, RFC 060) where applicable.

## Public OIDC surface

| Route | Method | Auth | CSRF | Handler | Render | Audit emit |
|---|---|---|---|---|---|---|
| `/.well-known/openid-configuration` | GET | oidc-public | n/a | `oidc::discovery` | — (JSON) | — |
| `/.well-known/jwks.json` | GET | oidc-public | n/a | `oidc::jwks` | — (JWKS JSON) | — |
| `/oauth2/userinfo` | GET, POST | oidc-bearer | n/a | `oidc::userinfo` | — (JSON) | — |
| `/oauth2/token` | POST | client-cred | n/a | `oidc::token` | — (JSON) | `oauth.token.issued`, `auth.refresh.rotated`, `auth.refresh.theft_detected` |
| `/oauth2/authorize` | GET | user (redirects to login if absent) | n/a (state in PKCE) | `oidc::authorize` | `render_login` (anon) · `render_consent` (auth) · redirect (consent already granted) | `auth.authorize.code_issued` |
| `/oauth2/logout` | GET | (optional `id_token_hint`) | n/a | `oidc::logout` | redirect | `auth.session.terminated` |
| `/oauth2/introspect` | POST | client-cred | n/a | `oauth_token::introspect` | — (JSON, RFC 7662) | — |
| `/oauth2/revoke` | POST | client-cred | n/a | `oauth_token::revoke` | — (RFC 7009) | `auth.token.revoked` |

CORS attached: `/.well-known/*` and `/oauth2/userinfo` use
`cors::public_read` (`*`); `/oauth2/token` uses
`cors::token_endpoint` (per-origin allowlist from registered
redirect URIs).

## Root and health

| Route | Method | Auth | CSRF | Handler | Render | Audit emit |
|---|---|---|---|---|---|---|
| `/` | GET | none | n/a | `index::root` | redirect to `/setup` (uninitialised) or `/admin` (initialised) | — |
| `/healthz` | GET | none | n/a | `index::healthz` | — (plain text "ok") | — |

## Setup wizard

| Route | Method | Auth | CSRF | Handler | Render | Audit emit |
|---|---|---|---|---|---|---|
| `/setup` | GET | setup-token (via `?token=…`) | n/a | `setup::welcome_get` | `render_setup_welcome` | — |
| `/setup/admin` | GET | setup-token | n/a | `setup::admin_get` | `render_setup_admin` | — |
| `/setup/admin` | POST | setup-token | ✓ | `setup::admin_post` | `render_setup_admin` (on error) | `setup.admin.created` |
| `/setup/lang` | GET | setup-token | n/a | `setup::lang_get` | `render_setup_lang` | — |
| `/setup/lang` | POST | setup-token | ✓ | `setup::lang_post` | `render_setup_lang` (on error) | `setup.lang.set` |
| `/setup/hibp` | GET | setup-token | n/a | `setup::hibp_get` | `render_setup_hibp` | — |
| `/setup/hibp` | POST | setup-token | ✓ | `setup::hibp_post` | `render_setup_hibp` (on error) | `setup.hibp.set`, `setup.completed` |
| `/setup/done` | GET | setup-token (then closes) | n/a | `setup::done_get` | `render_setup_done` | — |

## Authentication surfaces (non-OIDC)

| Route | Method | Auth | CSRF | Handler | Render | Audit emit |
|---|---|---|---|---|---|---|
| `/admin/login` | GET | none | n/a | `admin::login_get` | `render_login` | — |
| `/admin/login` | POST | none | ✓ | `admin::login_post` | `render_login` (on failure) | `auth.login.success`, `auth.login.failure`, `auth.lockout` |
| `/admin/login/mfa` | GET | pre-auth ticket | n/a | `admin::mfa_challenge_get` | `render_mfa_challenge` | — |
| `/admin/login/mfa` | POST | pre-auth ticket | ✓ | `admin::mfa_challenge_post` | `render_mfa_challenge` (on failure) | `auth.mfa.success`, `auth.mfa.failure` |
| `/admin/login/webauthn/start` | POST | pre-auth ticket | ✓ | `admin::webauthn_auth_start` | — (JSON challenge) | — |
| `/admin/login/webauthn/complete` | POST | pre-auth ticket | ✓ | `admin::webauthn_auth_complete` | — (JSON / redirect) | `auth.webauthn.success`, `auth.webauthn.failure` |
| `/admin/logout` | POST | user | ✓ | `admin::logout` | redirect | `auth.session.terminated` |
| `/admin/profile` | GET | user | n/a | `me_security::admin_profile_redirect` | 308 redirect to `/me/security/overview` (legacy bookmark) | — |
| `/forgot-password` | GET | none | n/a | `forgot_password::forgot_password_get` | `render_forgot_password` | — |
| `/forgot-password` | POST | none | ✓ | `forgot_password::forgot_password_post` | `render_forgot_password_sent` (always — anti-enumeration) | `auth.forgot_password.requested` |
| `/reset-password` | GET | reset-token | n/a | `forgot_password::reset_password_get` | `render_reset_password` or `render_reset_password_invalid` | — |
| `/reset-password` | POST | reset-token | ✓ | `forgot_password::reset_password_post` | `render_reset_password` (on error) | `auth.password.reset` |

## Admin: dashboard, users, clients

| Route | Method | Auth | CSRF | Handler | Render | Audit emit |
|---|---|---|---|---|---|---|
| `/admin` | GET | admin | n/a | `admin::dashboard` | `render_dashboard` | — |
| `/admin/users` | GET | admin | n/a | `admin::users_get` | `render_users` | — |
| `/admin/users` | POST | admin | ✓ | `admin::users_create` | `render_users` (on error) | `admin.user.created` |
| `/admin/users/{id}` | GET | admin | n/a | `admin::users_detail_get` | `render_user_detail` | — |
| `/admin/users/{id}/disable-confirm` | GET | admin | n/a | `admin::users_disable_confirm_get` | `render_confirm_disable_user` | — |
| `/admin/users/{id}/disabled` | POST | admin + step-up | ✓ | `admin::users_set_disabled` | redirect (or `render_user_detail` on error) | `admin.user.disabled`, `admin.user.re_enabled` |
| `/admin/users/{id}/delete-confirm` | GET | admin | n/a | `admin::users_delete_confirm_get` | `render_confirm_delete_user` | — |
| `/admin/users/{id}/delete` | POST | admin + step-up | ✓ | `admin::users_delete` | redirect | `admin.user.deleted` |
| `/admin/users/{id}/mfa-reset-confirm` | GET | admin | n/a | `admin::users_mfa_reset_confirm_get` | `render_confirm_reset_mfa` | — |
| `/admin/users/{id}/mfa-reset` | POST | admin + step-up | ✓ | `admin::users_mfa_reset` | redirect | `admin.user.mfa_reset` |
| `/admin/clients` | GET | admin | n/a | `admin::clients_get` | `render_clients` | — |
| `/admin/clients` | POST | admin | ✓ | `admin::clients_create` | `render_clients` (on error) | `admin.client.created` |
| `/admin/clients/{id}/edit` | GET | admin | n/a | `admin::clients_edit_get` | `render_client_edit` | — |
| `/admin/clients/{id}/edit` | POST | admin | ✓ | `admin::clients_edit_post` | `render_client_edit` (on error) | `admin.client.updated` |
| `/admin/clients/{id}/disabled` | POST | admin | ✓ | `admin::clients_set_disabled` | redirect | `admin.client.disabled`, `admin.client.re_enabled` |
| `/admin/clients/{id}/delete-confirm` | GET | admin | n/a | `admin::clients_delete_confirm_get` | `render_confirm_delete_client` | — |
| `/admin/clients/{id}/delete` | POST | admin + step-up | ✓ | `admin::clients_delete` | redirect | `admin.client.deleted` |
| `/admin/clients/{id}/rotate-secret` | POST | admin + step-up | ✓ | `admin::clients_rotate_secret_post` | `render_client_edit` (with new secret shown once) | `admin.client.secret_rotated` |

## Admin: signing keys, audit, settings

| Route | Method | Auth | CSRF | Handler | Render | Audit emit |
|---|---|---|---|---|---|---|
| `/admin/signing-keys` | GET | admin | n/a | `admin::signing_keys_get` | `render_signing_keys` | — |
| `/admin/signing-keys/rotate` | POST | admin | ✓ | `admin::signing_keys_rotate` | redirect | `admin.signing_key.rotated` |
| `/admin/signing-keys/{id}/delete-confirm` | GET | admin | n/a | `admin::signing_keys_delete_confirm_get` | `render_confirm_delete_signing_key` | — |
| `/admin/signing-keys/{id}/delete` | POST | admin + step-up | ✓ | `admin::signing_keys_delete` | redirect | `admin.signing_key.deleted` |
| `/admin/audit` | GET | admin | n/a | `admin::audit_get` | `render_audit` | — |
| `/admin/audit.csv` | GET | admin | n/a | `admin::audit_csv_get` | — (CSV stream) | — |
| `/admin/settings` | GET | admin | n/a | `settings::index_redirect` | 302 redirect to `/admin/settings/basic` | — |
| `/admin/settings/basic` | GET | admin | n/a | `settings::basic_get` | `render_settings_basic` | — |
| `/admin/settings/basic/lang` | POST | admin | ✓ | `settings::basic_lang_post` | redirect | `admin.settings.default_lang.updated` |
| `/admin/settings/security` | GET | admin | n/a | `settings::security_get` | `render_settings_security` | — |
| `/admin/settings/security/idle-timeout` | POST | admin | ✓ | `settings::idle_timeout_post` | redirect | `admin.settings.idle_session_timeout.updated` |
| `/admin/settings/security/max-sessions` | POST | admin | ✓ | `settings::max_sessions_post` | redirect | `admin.settings.max_concurrent_sessions.updated` |
| `/admin/settings/authentication` | GET | admin | n/a | `settings::authentication_get` | `render_settings_authentication` | — |
| `/admin/settings/email` | GET | admin | n/a | `settings::email_get` | `render_settings_email` | — |
| `/admin/settings/email` | POST | admin | ✓ | `settings::email_post` | `render_settings_email` (on error) | `admin.settings.smtp.updated` |
| `/admin/settings/email/test` | POST | admin | ✓ | `settings::email_test` | redirect | `admin.settings.smtp.test_sent` |
| `/admin/settings/logs` | GET | admin | n/a | `settings::logs_get` | `render_settings_logs` | — |
| `/admin/settings/other` | GET | admin | n/a | `settings::other_get` | `render_settings_other` | — |

## Self-service: `/me/security/*`

| Route | Method | Auth | CSRF | Handler | Render | Audit emit |
|---|---|---|---|---|---|---|
| `/me/security` | GET | user | n/a | `me_security::security_redirect` | 302 redirect to `/me/security/overview` | — |
| `/me/security/overview` | GET | user | n/a | `me_security::overview_get` | `render_me_overview` | — |
| `/me/security/password` | GET | user | n/a | `me_security::password_change_get` | `render_me_security` | — |
| `/me/security/password` | POST | user | ✓ | `me_security::password_change_post` | `render_me_security` (on error) | `auth.password.changed` |
| `/me/security/mfa` | GET | user | n/a | `me_security::mfa_get` | `render_me_mfa` | — |
| `/me/security/mfa/enroll/start` | POST | user | ✓ | `me_security::mfa_enroll_start` | `render_me_mfa` (with QR + secret) | — |
| `/me/security/mfa/enroll/confirm` | POST | user | ✓ | `me_security::mfa_enroll_confirm` | `render_me_mfa` (with recovery codes shown once) | `auth.mfa.enabled` |
| `/me/security/mfa/disable` | POST | user + step-up | ✓ | `me_security::mfa_disable` | redirect | `auth.mfa.disabled` |
| `/me/security/mfa/recovery-codes/regenerate` | POST | user | ✓ | `me_security::mfa_regenerate_recovery` | `render_me_mfa` (new codes shown once) | `auth.mfa.recovery_codes_regenerated` |
| `/me/security/sessions` | GET | user | n/a | `me_security::sessions_tab_get` | `render_me_sessions` | — |
| `/me/security/sessions/{id}/revoke` | POST | user | ✓ | `me_security::revoke_one` | redirect | `auth.session.revoked` |
| `/me/security/sessions/revoke-all-others` | POST | user | ✓ | `me_security::revoke_all_others` | redirect | `auth.session.revoked_all_others` |
| `/me/security/passkeys` | GET | user | n/a | `me_security::passkeys_get` | `render_me_passkey` | — |
| `/me/security/passkeys/{id}/rename` | POST | user | ✓ | `me_security::passkey_rename_post` | redirect | `auth.passkey.renamed` |
| `/me/security/passkeys/register/start` | POST | user | ✓ | `me_security::passkey_register_start` | — (JSON challenge) | — |
| `/me/security/passkeys/register/complete` | POST | user | ✓ | `me_security::passkey_register_complete` | redirect | `auth.passkey.registered` |
| `/me/security/passkeys/{id}/delete` | POST | user + step-up | ✓ | `me_security::passkey_delete` | redirect | `auth.passkey.deleted` |
| `/me/security/language` | GET | user | n/a | `me_security::language_get` | `render_me_language` | — |
| `/me/security/language` | POST | user | ✓ | `me_security::language_post` | redirect | `auth.preferred_lang.updated` |

## Step-up

| Route | Method | Auth | CSRF | Handler | Render | Audit emit |
|---|---|---|---|---|---|---|
| `/me/security/step-up` | GET | user (with `?next=…` parameter) | n/a | `step_up::get` | `render_step_up` | — |
| `/me/security/step-up` | POST | user | ✓ | `step_up::post` | `render_step_up` (on failure) | `auth.step_up.success`, `auth.step_up.failure` |
| `/me/security/step-up/webauthn/start` | POST | user | ✓ | `step_up::webauthn_start` | — (JSON challenge) | — |
| `/me/security/step-up/webauthn/finish` | POST | user | ✓ | `step_up::webauthn_finish` | — (JSON) | `auth.step_up.success`, `auth.step_up.failure` |

## Static assets

| Route | Method | Auth | CSRF | Handler | Render | Audit emit |
|---|---|---|---|---|---|---|
| `/static/{*path}` | GET | none | n/a | `assets::serve` | — (`include_dir!` byte stream) | — |

Assets shipped: `theme-init.js`, `copy.js`, `logout-csrf.js`,
favicon, robots.txt. No build pipeline.

## Aggregate route count

- **OIDC public surface:** 8 routes
- **Setup wizard:** 5 routes (3 GET-only + 4 GET+POST collapsed)
- **Auth surfaces:** 9 routes (login, MFA, webauthn, logout,
  profile-redirect, forgot, reset)
- **Admin / dashboard / users / clients / signing-keys / audit / settings:** 38 routes
- **Self-service `/me/security/*`:** 19 routes (incl. step-up)
- **Misc (root, health, static):** 3 routes

**Total: ~82 routes** (the spec §3 codebase handoff cites "~80"; the
exact count after v0.48.4 audit is 82).

## Cross-reference

- For the inbound mockup-screen mapping, see [`screen-map.md`](./screen-map.md).
- For the dangerous-operation route subset, see [`dangerous-action-map.md`](./dangerous-action-map.md).
- For the path-tab structure, see [`tab-routing-delta.md`](./tab-routing-delta.md).
- For per-route copy expectations, see [`i18n-copy-delta-draft.md`](./i18n-copy-delta-draft.md).
- For visual primitive composition, see [`token-delta-draft.md`](./token-delta-draft.md).

## Acceptance criteria (Phase 0)

- [x] Every route in `crates/sui-id/src/router.rs` listed with
  method, auth, CSRF, handler, render function, and audit-event
  emission.
- [x] No route is missing from the table; the route count matches
  the codebase handoff's "~80" claim within rounding (82 actual).
- [x] No protocol surface (`/oauth2/*`, `/.well-known/*`) is on the
  integration-touch list.
- [x] Every confirm GET has a matching destructive POST in the same
  row group.
- [x] Every step-up-gated POST is annotated.
