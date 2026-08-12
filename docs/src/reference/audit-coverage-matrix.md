# Audit Coverage Matrix (RFC 085)

This document is the **normative requirement** for sui-id's audit event coverage:
every privileged operation that mutates state **must** have a row here specifying
its event name, required fields, and atomicity class.

**That requirement is not mechanically enforced today.** The CI gate
`scripts/check-audit-matrix.sh` compares *event-name strings* between this
document and the code. It does not, and cannot, establish that a privileged
operation emits an event at all, that the event is correctly typed, or that the
mutation and its audit append share a transaction.

> **Diagnostic only: string parity proves neither emission completeness nor
> mutation/audit atomicity. RFC 094 owns the authoritative structural gate.**

Until RFC 094 is implemented, treat the rows below as the coverage this project
*requires*, verified by review rather than by CI.

## Atomicity classes

| Class | Guarantee |
|---|---|
| **A — atomic** | State change and audit row commit in one SQLite transaction, or neither does. A crash between them is impossible by construction (`audit::append_within_tx`). |
| **B — best-effort** | Audit append is attempted; a failure is logged loudly but does not suppress the primary security response (revocation, denial). |

## Coverage matrix

### User management (`user.*`)

| Event name | Operation | Actor | Target | Note fields | Class |
|---|---|---|---|---|---|
| `user.create` | Create user | admin user id | new user id | — | A |
| `user.create_warned_hibp` | Create user (HIBP breach warning) | admin user id | new user id | — | A |
| `user.disable` | Disable user account | admin user id | target user id | reason (optional) | A |
| `user.enable` | Re-enable user account | admin user id | target user id | — | A |
| `user.delete` | Soft-delete user | admin user id | target user id | reason (optional) | A |
| `user.reset_password` | Admin password reset | admin user id | target user id | — | A |
| `user.reset_mfa` | Admin MFA reset | admin user id | target user id | `totp=… passkeys=N reason=…` | A |

> **Conversion pending (RFC 085):** user.role_change will be added when the role-change handler is converted to Class A atomicity.

### Client management (`client.*`)

| Event name | Operation | Actor | Target | Note fields | Class |
|---|---|---|---|---|---|
| `client.create` | Create OIDC client | admin user id | new client id | — | A |
| `client.update` | Update client basic info | admin user id | client id | — | A |
| `client.set_allowed_scopes` | Set client allowed scopes | admin user id | client id | — | A |
| `client.set_post_logout_redirect_uris` | Set post-logout URIs | admin user id | client id | — | A |
| `client.disable` | Disable client | admin user id | client id | — | A |
| `client.enable` | Re-enable client | admin user id | client id | — | A |
| `client.delete` | Soft-delete client | admin user id | client id | reason (optional) | A |
| `client.rotate_secret` | Rotate client secret | admin user id | client id | — | A |

### Signing keys (`signing_key.*`)

| Event name | Operation | Actor | Target | Note fields | Class |
|---|---|---|---|---|---|
| `signing_key.rotate` | Issue new signing key | admin user id | new key id | — | A |
| `signing_key.delete` | Delete signing key | admin user id | key id | — | A |

### Administrative (`admin.*`)

| Event name | Operation | Actor | Target | Note fields | Class |
|---|---|---|---|---|---|
| `admin.master_key.rotated` | Master key rotation | CLI principal | — | `keys_resealed=N` | A |
| `admin.user.unlock` | Clear account lockout | admin user id | target user id | — | A |

### Pending settings changes (`settings.pending_change.*`)

| Event name | Operation | Actor | Target | Note fields | Class |
|---|---|---|---|---|---|
| `settings.pending_change.created` | Pending change stored | admin user id | — | `intent=… id=… summary=…` | B |
| `settings.pending_change.applied` | Pending change applied | admin user id | — | `intent=… summary=…` | B |
| `settings.pending_change.cancelled` | Pending change cancelled | admin user id | — | `id=…` | B |
| `settings.pending_change.binding_failed` | Binding check failed on apply | admin user id | — | `intent=… id=…` | B |

### Federation sign-in (`auth.federation.*`, RFC 004)

| Event name | Operation | Actor | Target | Note fields | Class |
|---|---|---|---|---|---|
| `auth.federation.signin.success` | Federated sign-in completed | user id | user id | `provider=… sub=…` | B |
| `auth.federation.signin.upstream_failure` | Upstream IdP returned an error | — | — | `provider=… error=…` | B |
| `auth.federation.link.created` | Federation link created (first sign-in or explicit link) | user id | user id | `provider=… sub=…` | B |
| `auth.federation.takeover_blocked` | Email collision rejected as potential takeover | — | — | `provider=… email=…` | A |

### Dynamic client registration (`client.dynamic_register`, RFC 008)

| Event name | Operation | Actor | Target | Note fields | Class |
|---|---|---|---|---|---|
| `client.dynamic_register` | RFC 7591 dynamic client registration | — | new client id | `name=…` | B |

### External user-source authentication (`auth.user_source.*`, RFC 005)

| Event name | Operation | Actor | Target | Note fields | Class |
|---|---|---|---|---|---|
| `auth.user_source.matched` | External source authenticated a user | shadow user id | shadow user id | `source=… stable_id=…` | B |
| `auth.user_source.transport_failure` | Directory unreachable during cascade | — | — | `source=… error=…` | B |

### Self-service settings (`auth.smtp_config.*`)

| Event name | Operation | Actor | Target | Note fields | Class |
|---|---|---|---|---|---|
| `auth.smtp_config.changed` | SMTP configuration change | admin user id | — | changed fields (non-secret) | A |

### Authentication flow (`auth.*`)

These are informational trace events (Class B) recording the authentication
funnel; they do not guard state mutations.

| Event name | Trigger | Actor | Class |
|---|---|---|---|
| `auth.login` | Login attempt initiated | — | B |
| `auth.login.password_ok_mfa_required` | Password correct; MFA challenge pending | user id | B |
| `auth.login.success` | Login succeeded | user id | B |
| `auth.login.failure` | Wrong password | — | B |
| `auth.login.locked` | Account locked at login | — | B |
| `auth.login.password_ok_mfa_required` | Password correct; MFA challenge pending | user id | B |
| `auth.mfa.success` | MFA challenge passed | user id | B |
| `auth.mfa.failure` | MFA challenge failed | user id | B |
| `auth.logout` | Session logout | user id | B |
| `auth.lockout` | Account auto-locked after failures | — | B |
| `auth.session.revoked` | Single session revocation | user id | B |
| `auth.sessions.bulk_revoke_self` | Bulk session revocation (self) | user id | B |
| `auth.password.changed_self` | Self-service password change | user id | B |
| `auth.password.reset_requested` | Forgot-password flow started | — | B |
| `auth.password.reset_email_sent` | Reset email dispatched | — | B |
| `auth.password.reset_email_failed` | Reset email failed to send | — | B |
| `auth.password.reset_throttled` | Reset request throttled | — | B |
| `auth.password.reset_completed` | Password reset completed | user id | B |
| `auth.refresh.theft_detected` | Replay of a rotated refresh token (family revoked) | user id | B |

### OAuth2 / OIDC (`oauth2.*`)

| Event name | Trigger | Actor | Class |
|---|---|---|---|
| `oauth2.exchange_code.user_revoked` | User disabled between authorization and token exchange | user id | B |

## Event name conventions

- Dot-delimited, lowercase: `<resource>.<verb>` or `<subsystem>.<event>`.
- `result` field: always `"ok"` for success; `"denied"` for security events.
- `note` fields **must not** contain raw secrets. RFC 078 typed newtypes
  (e.g. `RawRefreshToken`) do not implement `Display`, so accidental
  interpolation is a compile error. Audit notes carry only non-secret
  summaries (counts, IDs, sanitised reasons).
- Class-A operations use `audit::append_within_tx` (RFC 085) so the state
  change and audit row are atomic. Class-B operations use `audit::append`
  (best-effort async).

## CI gate

`scripts/check-audit-matrix.sh` compares two sets of **event-name strings**:

1. Every event name in this matrix exists as a string literal in the codebase.
2. Every audit-namespaced string literal in the codebase has a row in this matrix.

A mismatch between those two sets fails the gate. That is the whole of what it
checks.

**What it therefore catches:** a newly introduced audit-namespaced event-name
literal with no matrix row, or a matrix row naming an event no longer present in
the code.

**What it does not catch**, and must not be described as catching:

- a privileged operation that emits **no** audit event — there is no literal to
  compare, so the gate passes;
- an operation emitting a literal that does not match the audit namespace;
- an event whose name matches but whose payload, fields or atomicity class are
  wrong;
- an audit append that happens **outside** its mutation's transaction.

A live example of the limits of name-based comparison: the known
`user.reset_mfa` / `mfa.admin_reset` mismatch.

**Adding a new privileged operation without updating the matrix is therefore not
necessarily a CI failure.** It is a violation of the requirement stated at the top
of this document, caught by review. RFC 094 replaces this diagnostic with a
structural gate that can enforce it.
