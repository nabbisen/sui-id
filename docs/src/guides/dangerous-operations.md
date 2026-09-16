# Dangerous operations

> **Scope.** This guide covers the eight operations sui-id classifies
> as **dangerous** — actions that meaningfully reduce the security or
> availability of a user, client, or signing key. Each one goes
> through the same four-step contract: a confirm screen, a step-up
> re-authentication, the action itself, and a populated audit-log row.

## The four-step contract (RFC 030 + RFC 058 + RFC 060)

Every dangerous action is gated by:

1. **Confirm screen.** A separate page is shown first, explaining
   what is about to happen and what is reversible. It carries a
   `_confirmed=1` hidden field; direct POSTs without this field are
   rejected with HTTP 400. The confirm screen is built from a single
   shared template (`ConfirmScreenData`) so every dangerous action has
   the same affordances: identity-of-target line, blast-radius
   summary, reversibility badge, optional reason textarea, cancel
   button.
2. **Step-up.** Immediately before the action runs, the server checks
   that the operator has completed a fresh re-authentication (within
   the last 5 minutes, a fixed constant). Stale sessions are redirected to
   `/me/security/step-up?return_to=…`
   and the action waits.
3. **The action.** Only after both gates pass does the use case
   function in `sui-id-core` execute.
4. **Audit row with note.** The action writes one row to the audit
   log with `result="ok"` and `note` populated by either the
   operator-supplied reason or a canonical short code (`"self"` for
   self-service routes; `"totp=removed passkeys=N"` for MFA reset).
   The reason is your forensic signal when triaging "why did this
   happen at 03:00 UTC."

## Operation catalogue

The table below lists the eight actions and what each gate does in
practice.

| Action | HTTP route | Reversible? | Audit action | What gets revoked along with the primary effect |
|--------|-----------|:-----------:|--------------|-----------------------------------------------|
| **Disable user** | `POST /admin/users/{id}/disabled` | yes | `user.disable` | All sessions, all refresh tokens, all in-flight authorisation codes for the target. |
| **Delete user** | `POST /admin/users/{id}/delete` | no | `user.delete` | Same as disable, plus the user row is soft-deleted (removed from listings, kept in audit trail). |
| **Reset another user's MFA** | `POST /admin/users/{id}/mfa-reset` | yes¹ | `mfa.admin_reset` | Both TOTP and every WebAuthn credential. Active sessions are **not** revoked; the operator is restoring login capability, not logging the user out. |
| **Disable client** | `POST /admin/clients/{id}/disabled` | yes | `client.disable` | All refresh tokens for the client. |
| **Delete client** | `POST /admin/clients/{id}/delete` | no | `client.delete` | All refresh tokens for the client; the client row is soft-deleted. Dependent applications stop validating tokens. |
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
   verbatim. If `note: "self"`, the user did this on their own
   account. If empty (`null`), the operator left the reason textarea
   blank — chase them up.
3. **Cross-reference step-up.** Step-up activity is logged as
   `auth.step_up.complete` rows. If a dangerous action ran without a
   recent step-up row from the same actor, something is wrong with
   the gate (file a bug).
4. **Look at the surrounding rows.** Dangerous actions usually come
   in clusters during planned maintenance (e.g. one operator
   disabled three users and rotated a key in a 90-second window
   during off-boarding). Isolated single rows at odd hours are the
   signal worth investigating.

## When a confirm screen is bypassed

The `_confirmed=1` requirement is server-side and cannot be turned
off. If you find a dangerous action that succeeded without going
through the confirm screen, it is a bug, not a configuration option:

1. Capture the request log (URL, headers, form body).
2. Cross-reference with `auth.step_up.complete` for the same actor.
3. File a security issue with the captured details.

The four-step contract has no escape hatch. Operators who need to
script bulk operations should use the OIDC management endpoints (if
implemented for the action) or write a one-off script using the
internal use-case functions in `sui-id-core::admin` — those bypass
the step-up gate because they're trusted code, but they still write
the same audit-log row.

## Related references

- RFC 030 — Dangerous-action confirmation gate
- RFC 045 — Operator-supplied reason on disable
- RFC 058 — Step-up enforcement on the four previously unguarded routes
- RFC 059 — `<ConfirmScreen>` template component
- RFC 060 — Audit-note rollout
- [Audit event reference](../reference/audit-events.md) — the canonical
  list of action strings and what each one means.
