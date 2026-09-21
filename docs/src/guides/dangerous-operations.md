# Dangerous operations

> **Scope.** This guide covers the eight operations sui-id classifies
> as **dangerous** — actions that meaningfully reduce the security or
> availability of a user, client, or signing key. Each one goes
> through the same four-step contract: a confirm screen, a step-up
> re-authentication, the action itself, and a populated audit-log row.

## The four-step contract (RFC 030 + RFC 058 + RFC 060)

Every dangerous action is gated by:

1. **Confirm screen.** Six of the eight operations — disable user,
   delete user, reset MFA, delete client, rotate signing key and delete
   signing key — show a separate page first, explaining what is about
   to happen and what is reversible. Those pages share one template
   (`ConfirmScreenData`) with the same affordances: identity-of-target
   line, blast-radius summary, reversibility badge, optional reason
   textarea, cancel button. Client disable and client secret rotation
   have no confirm screen.

   All eight POST handlers require a `_confirmed=1` field and reject a
   request without it with HTTP 400. The Disable / Enable button on the
   client list does not send that field, so disabling or enabling a
   client from the admin panel currently fails with HTTP 400. The admin
   panel has no control for client secret rotation.
2. **Step-up.** Immediately before the action runs, the server checks
   that the operator has completed a fresh re-authentication (within
   the last 5 minutes, a fixed constant). Stale sessions are redirected to
   `/me/security/step-up?return_to=…`
   and the action waits. An operator whose account has no second factor
   passes this gate without a challenge. For the four operations recorded
   atomically (disable user, delete user, reset MFA, rotate signing key) the
   check is repeated inside the transaction that commits the action; if the
   step-up lapsed in between, nothing is changed and the operator is sent to
   step up again.
3. **The action.** Only after both gates pass does the use case
   function in `sui-id-core` execute.
4. **Audit row with note.** The action writes one row to the audit
   log with `result="ok"`. For disable user, delete user, reset MFA and
   rotate signing key, the row is committed in the same transaction as the
   action, and its note is a list of `key=value` fields: the
   operator-supplied reason as `reason=…` when one was given, and
   `step_up=…`, which records what authorized the action (see below). For the
   other four operations the row is written after the action and its note is
   the operator-supplied reason alone; on the self-service routes it is the
   fixed notes listed below. The reason is your forensic signal when
   triaging "why did this happen at 03:00 UTC."

## Operation catalogue

The table below lists the eight actions and what each gate does in
practice.

| Action | HTTP route | Reversible? | Audit action | What gets revoked along with the primary effect |
|--------|-----------|:-----------:|--------------|-----------------------------------------------|
| **Disable user** | `POST /admin/users/{id}/disabled` | yes | `user.disable` | All sessions, all refresh tokens, all in-flight authorisation codes for the target. |
| **Delete user** | `POST /admin/users/{id}/delete` | no | `user.delete` | Same as disable, plus the user row is soft-deleted (removed from listings, kept in audit trail). |
| **Reset another user's MFA** | `POST /admin/users/{id}/mfa-reset` | yes¹ | `mfa.admin_reset` | Both TOTP and every WebAuthn credential. Active sessions are **not** revoked; the operator is restoring login capability, not logging the user out. |
| **Disable client** | `POST /admin/clients/{id}/disabled` | yes | `client.disable` | All refresh tokens for the client. |
| **Delete client** | `POST /admin/clients/{id}/delete` | no | `client.delete` | All refresh tokens for the client; the client row is soft-deleted. Access and ID tokens already issued are not invalidated: relying parties validate them against JWKS, not the client row. |
| **Rotate client secret** | `POST /admin/clients/{id}/rotate-secret` | no² | `client.rotate_secret` | The old secret hash is replaced. Any application configured with the previous secret will fail authentication until reconfigured. The new plaintext secret is shown once on the response page. |
| **Rotate signing key** | `POST /admin/signing-keys/rotate` | yes³ | `signing_key.rotate` | A new active key is generated; the previous key is retired but kept in JWKS so already-issued tokens remain valid until expiry. |
| **Delete signing key** | `POST /admin/signing-keys/{id}/delete` | no | `signing_key.delete` | The retired key row is permanently removed. **Will refuse** to delete the currently active key (rotate first). |

¹ "Reversible" in the sense that the user can re-enrol; no permanent
data is lost. But once removed, the codes/passkeys can't be put back.

² "Reversibility" of secret rotation depends on whether the new
plaintext is captured at rotation time. The plaintext is shown once
on the success page and never stored.

³ Old key rows live until you delete them; "rotation" is reversible
to the extent that you can keep both keys published.

## Self-service dangerous actions

Three actions on `/me/security/*` reduce the user's own account
security. They follow the same step-up contract but write
`note: "self"` to the audit log so you can distinguish "user did this
themselves" from "an admin did it":

| Action | HTTP route | Audit action | Audit note |
|--------|-----------|--------------|------------|
| **Disable own MFA** | `POST /me/security/mfa/disable` | `mfa.disable` | `"self"` |
| **Delete own passkey** | `POST /me/security/passkeys/{id}/delete` | `webauthn.credential.delete` | `"self"` |
| **Revoke other sessions** | `POST /me/security/sessions/revoke-all-others` | `auth.sessions.bulk_revoke_self` | `revoked N other session(s)` |

These actions don't prompt for a reason — that would be friction on
your own account — but the note field still distinguishes the path
clearly in the audit log.

## Triaging an unexpected dangerous-action row

When a dangerous row appears in `/admin/audit` and you don't know
why:

1. **Identify the actor.** The `actor` column is the user ID of
   whoever clicked the button. For self-service rows, actor and
   target are the same.
2. **Read the note.** If the operator typed a reason, it's there
   verbatim (as the `reason=` field for disable, delete, MFA reset and
   key rotation, whose note also ends in `step_up=…`; as the whole note
   for the others). If `note: "self"`, the user did this on their own
   account. If empty (`null`), the operator left the reason textarea
   blank — chase them up.
3. **Look at the surrounding rows.** Dangerous actions usually come
   in clusters during planned maintenance (e.g. one operator
   disabled three users and rotated a key in a 90-second window
   during off-boarding). Isolated single rows at odd hours are the
   signal worth investigating.
4. **Check what authorized it.** Every successful step-up is recorded as
   `auth.step_up.success`, with the `method` (`totp` or `webauthn`) and
   the `gate`, the page it was for. For disable user, delete user, reset MFA
   and rotate signing key, the action's own row says what authorized it in a
   `step_up=` field: `fresh:<method>:<seconds>` (the operator stepped up that
   many seconds earlier), `not_required:no_second_factor` (the operator's
   account has none), or, for `sui-id admin reset-mfa` run from the host,
   `not_applicable:system_principal` (no session, no actor). A row whose
   value is `not_required` is an administrator acting with no second factor,
   and is worth a question. The other four operations, and the self-service
   actions, do not carry the field; to see whether a step-up preceded one,
   look for an `auth.step_up.success` row by the same actor just before it.
   See [Reading `step_up=`](operators.md#reading-step_up) for the queries.

## When a confirm screen is bypassed

The `_confirmed=1` requirement is server-side and cannot be turned
off. If you find a dangerous action that succeeded without going
through the confirm screen, it is a bug, not a configuration option:

1. Capture the request log (URL, headers, form body).
2. File a security issue with the captured details.

The four-step contract has no escape hatch. Operators who need to
script bulk operations should use the OIDC management endpoints (if
implemented for the action) or write a one-off script using the
internal use-case functions in `sui-id-core::admin`. The HTTP step-up gate is
not part of those functions, but each takes an administrator actor bound to a
session, the four operations recorded atomically re-check that session's
step-up inside their transaction, and every one of them still writes its
audit-log row.

## Related references

- RFC 030 — Dangerous-action confirmation gate
- RFC 045 — Operator-supplied reason on disable
- RFC 058 — Step-up enforcement on the four previously unguarded routes
- RFC 059 — `<ConfirmScreen>` template component
- RFC 060 — Audit-note rollout
- RFC 102 — Authentication that cannot be audited does not succeed (step-up
  completion and its evidence)
- [Audit event reference](../reference/audit-events.md) — the canonical
  list of action strings and what each one means.
