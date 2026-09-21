# Audit event reference

Every action that sui-id records in the audit log uses a stable,
dot-separated lowercase event name. These names are safe to use in
log-search queries and SIEM rules — they will not change between releases.

The audit log is append-only and protected by a SHA-256 hash chain.
Use **Admin panel → Audit log** to filter by event prefix or export to CSV.

## Authentication events

| Event name | Label | Description |
|---|---|---|
| `auth.login.success` | Login | A sign-in that needs no second factor succeeded: a local password, or a user-source (LDAP) password. Note fields: `evicted`; for a user-source sign-in also `source` and `stable_id`. A user with a second factor completes with `auth.mfa.success` instead. |
| `auth.login.failure` | Login failed | The password did not match (counted toward a lockout; note field `count`), or the attempt was refused before any credential check: unknown username, disabled or deleted account, locked account (the note says which). |
| `auth.lockout` | Account locked | A wrong-password attempt just crossed the progressive-lockout failure threshold. |
| `auth.login.password_ok_mfa_required` | MFA required | Password was correct but MFA challenge is pending. |
| `auth.mfa.success` | MFA verified | A second factor completed the sign-in: a TOTP code, a recovery code or a passkey assertion. Note fields: `method` (`totp`, `recovery_code`, `webauthn`) and `evicted`. |
| `auth.mfa.failure` | MFA failed | TOTP code, recovery code or passkey assertion failed verification at sign-in; the note carries the consecutive `count` for the user. |
| `auth.mfa.lockout` | MFA lockout | The fifth consecutive wrong second factor: pending sign-ins removed and the account locked. |
| `auth.password.changed_self` | Password changed | User changed their own password via `/me/security/password`. |
| `auth.password.reset_requested` | Password reset requested | User submitted the forgot-password form. |
| `auth.password.reset_email_sent` | Reset email sent | A password-reset email was queued successfully. |
| `auth.password.reset_email_failed` | Reset email failed | The password-reset email could not be delivered. |
| `auth.password.reset_throttled` | Reset throttled | The forgot-password rate limit was reached for this address. |
| `auth.password.reset_completed` | Password reset | Password was successfully changed via a reset link. The note records `origin=` — `email` (the forgot-password flow), `web` (issued by an administrator) or `cli` (issued by the operator). |
| `auth.refresh.rotated` | Refresh token rotated | A refresh token was exchanged for a fresh access/refresh token pair (the normal, routine case). |
| `auth.refresh.theft_detected` | Token theft detected | A refresh token was presented that had already been rotated, indicating a possible token theft. The entire token family is revoked. |
| `auth.sessions.bulk_revoke_self` | All other sessions revoked | User revoked all sessions except the current one. |
| `auth.smtp_config.changed` | SMTP config changed | Administrator saved new SMTP settings. |

## User management events

The step-up-gated actions — disabling, enabling or deleting a user, resetting a
user's MFA, and rotating the signing key — record what authorized them in a
`step_up` note field, written in the same transaction as the action:

- `fresh:<method>:<seconds>`: the administrator stepped up with `method`
  (`totp` or `webauthn`) that many seconds earlier;
- `not_required:no_second_factor`: the administrator has no second factor, so no
  step-up was possible;
- `not_applicable:system_principal`: the operator ran `sui-id admin reset-mfa`,
  with no session (MFA reset only).

If the step-up has lapsed by the time the action commits, nothing is changed and
the administrator is asked to step up again.

| Event name | Label | Description |
|---|---|---|
| `user.create` | User created | Administrator created a new user account. |
| `user.create_warned_hibp` | — | Administrator created a user account whose password appears in a known breach (Have I Been Pwned), with the breach check in `warn` mode. |
| `user.disable` | User disabled | Administrator disabled a user account. All active sessions and refresh tokens are immediately revoked. |
| `user.enable` | User enabled | Administrator re-enabled a previously disabled user account. |
| `user.delete` | User deleted | Administrator deleted a user account. |
| `user.reset_password` | Password reset (admin) | Administrator reset a user's password. |
| `mfa.admin_reset` | MFA reset (admin) | An administrator (web), or the operator with `sui-id admin reset-mfa` (no actor, `via=cli`), reset a user's MFA factors (TOTP and all passkeys removed). |
| `user.recovery_link.issued` | Recovery link issued | An administrator (web), or the operator with `sui-id admin issue-recovery-link` (no actor, `via=cli`), issued a single-use account-recovery link for a user. The note records the `reason`, `via`, `expires_at`, how many of the user's earlier links this one `invalidated`, and `step_up`. The link itself is never recorded. |
| `admin.user.unlock` | Account unlocked | Administrator cleared a user's progressive lockout. |
| `user.role_change` | — | Administrator changed a user's role (admin, auditor or user). The mutation and this row commit in one transaction; the note records the old and new role. |

## Client management events

| Event name | Label | Description |
|---|---|---|
| `client.create` | Client created | Administrator registered a new OIDC client. |
| `client.update` | Client updated | Administrator updated an OIDC client's configuration. |
| `client.delete` | Client deleted | Administrator deleted an OIDC client. |
| `client.set_allowed_scopes` | Client scopes updated | Administrator changed the allowed scopes for a client. |
| `client.set_post_logout_redirect_uris` | — | Administrator changed the URIs a client may pass as `post_logout_redirect_uri` to `/oauth2/logout`. |
| `client.disable` | — | Administrator disabled an OIDC client. All of its refresh tokens are revoked. The note carries the reason, if one was given. |
| `client.enable` | — | Administrator re-enabled a previously disabled OIDC client. |
| `client.rotate_secret` | — | Administrator generated a new secret for a confidential client; the stored secret hash is replaced. Not available for public clients. |

## Signing key events

| Event name | Label | Description |
|---|---|---|
| `signing_key.rotate` | Signing key rotated | Administrator triggered a key rotation. A new Ed25519 key was generated and the previous key was retired. Note fields: `algorithm`, the operator's `reason` when given, and `step_up` (see the user management events above). |
| `signing_key.delete` | Signing key deleted | Administrator permanently deleted a retired signing key. |

## Infrastructure events

| Event name | Label | Description |
|---|---|---|
| `admin.master_key.rotated` | Master key rotated | The master key was rotated offline. All column-encrypted values were re-sealed under the new key. |
| `setup.create_initial_admin` | Initial admin created | The setup wizard completed and the first administrator account was created. |

## Using audit events in filters

The audit log filter (Admin panel → Audit log) matches by event prefix:

- `auth.login` → all login-related events
- `user.` → all user management events
- `auth.password` → all password-related events

The CSV export respects the same filter.

## Audit log integrity

Each row in the audit log contains a SHA-256 hash of its own content
concatenated with the previous row's hash (a hash chain). The Admin panel
verifies the chain tail on every load and shows a status banner:

- **✓ Audit chain verified** — no tampering detected in the checked rows.
- **✗ Audit chain integrity check failed** — a row hash does not match its
  recomputed value. Investigate immediately.

## Federation events (RFC 004)

| Event name | Label | Description |
|---|---|---|
| `auth.federation.signin.success` | Federated sign-in | User authenticated via an upstream OIDC provider and a local session was issued. Note fields: `provider` (the provider slug), `sub` (the upstream subject, truncated to 255 bytes), `evicted`. |
| `auth.federation.signin.upstream_failure` | Federation upstream error | The upstream identity provider returned an error during code exchange or discovery. |
| `auth.federation.link.created` | Federation link created | A new link between a local user account and an upstream identity was established (on first sign-in with `provision_on_first_login`, or via the explicit link flow). |
| `auth.federation.takeover_blocked` | Account takeover blocked | A federated sign-in was rejected because the upstream email matched an existing local user who is not linked to this provider (P2 — potential account takeover attempt). |

## Dynamic client registration events (RFC 008)

| Event name | Label | Description |
|---|---|---|
| `client.dynamic_register` | Dynamic client registered | A third-party application self-registered via `POST /oauth2/register` using a valid initial-access token. The client starts disabled; an administrator must enable it before it can obtain tokens. |

## External user-source events (RFC 005)

A sign-in through an external user source (LDAP) records `auth.login.success`,
with the source's slug in the `source` note field and the user's directory
stable id in `stable_id` (truncated to 255 bytes). A user with a second factor
completes with `auth.mfa.success`. Audit logs written before this change may
also hold rows of an older, separate user-source match event, which is no
longer written.

## Self-service MFA and passkey events

| Event name | Label | Description |
|---|---|---|
| `mfa.disable` | — | User turned off MFA from `/me/security/mfa`, a dangerous self-service action (RFC 058). |
| `webauthn.credential.delete` | — | User deleted one of their passkeys. Step-up authentication is required first (RFC 058). |
| `auth.mfa.factor_added` | — | User added a second factor: TOTP enrolment, fresh recovery codes, or a passkey. The `method` attribute says which. Recorded in the same transaction as the change. |
| `auth.step_up.success` | — | A step-up re-authentication succeeded on a signed-in session. `method` is `totp` or `webauthn`; `gate` is the page the step-up was for (truncated to 256 bytes). Recorded in the same transaction as the session's new freshness. |
| `auth.step_up.failure` | — | A step-up re-authentication failed on a signed-in session (wrong code, failed passkey assertion, or wrong password when adding a first second factor). The `count` attribute is the run of consecutive failures on that session. |
| `auth.step_up.session_revoked` | — | The fifth consecutive step-up failure on a session; that session was revoked. **Alert on this.** |

## Token endpoint events

| Event name | Label | Description |
|---|---|---|
| `oauth2.exchange_code.user_revoked` | — | An authorization code was presented for a user who was disabled or deleted after authorizing; the token exchange was refused. |
| `token.introspect` | — | A confidential client called `/oauth2/introspect` (RFC 7662). The row's result is `active` or `inactive`. |
| `token.revoke` | — | A confidential client called `/oauth2/revoke` (RFC 7009). |

## Pending settings change events (RFC 090)

High-risk settings changes that include a secret, such as new SMTP credentials,
are stored encrypted and applied only after the administrator confirms them.

| Event name | Label | Description |
|---|---|---|
| `settings.pending_change.created` | — | Administrator submitted a high-risk settings change; it was stored encrypted, awaiting confirmation. |
| `settings.pending_change.applied` | — | Administrator confirmed a pending settings change and it was applied. |
| `settings.pending_change.cancelled` | — | A pending settings change was cancelled before it was applied. |
| `settings.pending_change.binding_failed` | — | A pending settings change was refused on confirmation because a binding check (session, actor, CSRF or expiry) failed. |
