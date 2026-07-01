# Audit event reference

Every action that sui-id records in the audit log uses a stable,
dot-separated lowercase event name. These names are safe to use in
log-search queries and SIEM rules — they will not change between releases.

The audit log is append-only and protected by a SHA-256 hash chain.
Use **Admin panel → Audit log** to filter by event prefix or export to CSV.

## Authentication events

| Event name | Label | Description |
|---|---|---|
| `auth.login.success` | Login | User authenticated successfully with password (and MFA if enrolled). |
| `auth.login.failure` | Login failed | Credential check failed (wrong password or unknown username). |
| `auth.login.locked` | Account locked | Login refused because the account's progressive lockout has not expired. |
| `auth.login.password_ok_mfa_required` | MFA required | Password was correct but MFA challenge is pending. |
| `auth.logout` | Logout | User explicitly signed out. |
| `auth.mfa.success` | MFA verified | TOTP code or passkey assertion verified successfully. |
| `auth.mfa.failure` | MFA failed | TOTP code or passkey assertion failed verification. |
| `auth.password.changed_self` | Password changed | User changed their own password via `/me/security/password`. |
| `auth.password.reset_requested` | Password reset requested | User submitted the forgot-password form. |
| `auth.password.reset_email_sent` | Reset email sent | A password-reset email was queued successfully. |
| `auth.password.reset_email_failed` | Reset email failed | The password-reset email could not be delivered. |
| `auth.password.reset_throttled` | Reset throttled | The forgot-password rate limit was reached for this address. |
| `auth.password.reset_completed` | Password reset | Password was successfully changed via the reset link. |
| `auth.refresh.theft_detected` | Token theft detected | A refresh token was presented that had already been rotated, indicating a possible token theft. The entire token family is revoked. |
| `auth.session.revoked` | Session revoked | A single session was explicitly revoked (by user or admin). |
| `auth.sessions.bulk_revoke_self` | All other sessions revoked | User revoked all sessions except the current one. |
| `auth.smtp_config.changed` | SMTP config changed | Administrator saved new SMTP settings. |

## User management events

| Event name | Label | Description |
|---|---|---|
| `user.create` | User created | Administrator created a new user account. |
| `user.disable` | User disabled | Administrator disabled a user account. All active sessions and refresh tokens are immediately revoked. |
| `user.enable` | User enabled | Administrator re-enabled a previously disabled user account. |
| `user.delete` | User deleted | Administrator deleted a user account. |
| `user.reset_password` | Password reset (admin) | Administrator reset a user's password. |
| `mfa.admin_reset` | MFA reset (admin) | Administrator reset a user's MFA factors (TOTP and all passkeys removed). |
| `admin.user.unlock` | Account unlocked | Administrator cleared a user's progressive lockout. |

## Client management events

| Event name | Label | Description |
|---|---|---|
| `client.create` | Client created | Administrator registered a new OIDC client. |
| `client.update` | Client updated | Administrator updated an OIDC client's configuration. |
| `client.delete` | Client deleted | Administrator deleted an OIDC client. |
| `client.set_allowed_scopes` | Client scopes updated | Administrator changed the allowed scopes for a client. |

## Signing key events

| Event name | Label | Description |
|---|---|---|
| `signing_key.rotate` | Signing key rotated | Administrator triggered a key rotation. A new Ed25519 key was generated and the previous key was retired. |
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
| `auth.federation.signin.success` | Federated sign-in | User authenticated via an upstream OIDC provider and a local session was issued. |
| `auth.federation.signin.upstream_failure` | Federation upstream error | The upstream identity provider returned an error during code exchange or discovery. |
| `auth.federation.link.created` | Federation link created | A new link between a local user account and an upstream identity was established (on first sign-in with `provision_on_first_login`, or via the explicit link flow). |
| `auth.federation.takeover_blocked` | Account takeover blocked | A federated sign-in was rejected because the upstream email matched an existing local user who is not linked to this provider (P2 — potential account takeover attempt). |

## Dynamic client registration events (RFC 008)

| Event name | Label | Description |
|---|---|---|
| `client.dynamic_register` | Dynamic client registered | A third-party application self-registered via `POST /oauth2/register` using a valid initial-access token. The client starts disabled; an administrator must enable it before it can obtain tokens. |

## External user-source events (RFC 005)

| Event name | Label | Description |
|---|---|---|
| `auth.user_source.matched` | External user source matched | The login cascade resolved the user via an LDAP or other external user source. |
| `auth.user_source.transport_failure` | User source transport failure | A configured external user source (LDAP) was unreachable or returned a network error during authentication. |
