# RFC 030 — Dangerous operations: step-up + confirmation screens

**Status.** Implemented (v0.36.0)
**Priority.** High. Six dangerous operations currently use JavaScript
`confirm()` dialogs — a browser-native pattern that offers no step-up
authentication, no reversibility information, no impact summary, and
no audit trail of the operator's intent. This violates the core
"壊しにくい" (hard-to-break) principle from the design document.  
**Source.** UI/UX design document P.9, P.10, P.16; RFC 017 § 3.  
**Touches.** `crates/sui-id-web/src/pages.rs` (6 new confirmation screens),
`crates/sui-id/src/handlers/admin.rs` (GET routes for confirmation screens),
`crates/sui-id/src/router.rs` (6 new GET routes),
`crates/sui-id-i18n/src/strings.rs` (confirmation copy),
`crates/sui-id-web/src/components.rs` (reversibility badge — already added
in RFC 023).

## The six dangerous operations

| Operation | Reversible? | Current UI | Required UI |
|---|---|---|---|
| Disable user | Yes | `confirm()` | step-up + confirm screen |
| Delete user | No | `confirm()` | step-up + confirm screen |
| Reset user MFA | Yes (re-enrol) | `confirm()` | step-up + confirm screen |
| Force logout user | Yes (re-login) | `confirm()` | step-up + confirm screen |
| Regenerate client secret | No (breaks consumers) | `confirm()` | step-up + confirm screen |
| Delete client | No | `confirm()` | step-up + confirm screen |

## Confirmation screen contract (from RFC 017 § 3)

```
[Trigger button on list or detail page]
        │
        ▼
[Step-up challenge]     ← only if not within 5-minute freshness window
        │
        ▼
[Confirmation screen]
  · Identifies the target (username / client name)
  · States the impact concretely
      e.g. "This will revoke 3 active sessions for alice."
  · States reversibility
      e.g. "Disable can be undone. Delete cannot."
  · Reversibility badge (green "Recoverable" / red "Not recoverable")
      — colour is never the only signal; badge text always present
  · Primary button = explicit verb: "Disable alice" / "Delete Web App 1"
      — never "OK" or "Yes"
  · Cancel button → returns to originating list, no change
        │
  confirm│
        ▼
[Mutation executed; audit row written; flash on returning list page]
```

## Implementation

### New routes (GET → confirmation screen; POST → mutation)

```
GET  /admin/users/{id}/disable-confirm
GET  /admin/users/{id}/delete-confirm
GET  /admin/users/{id}/reset-mfa-confirm
GET  /admin/users/{id}/force-logout-confirm
GET  /admin/clients/{id}/delete-confirm
GET  /admin/clients/{id}/regenerate-secret-confirm
```

The existing `POST` mutation routes remain unchanged. The confirmation
screen `POST`s to the existing mutation endpoint with an extra hidden
`_confirmed=1` field (CSRF-protected); the mutation handler checks this
field and rejects the request if absent, preventing direct-POST bypasses.

### New render functions

```rust
pub fn render_user_disable_confirm(user: UserSummary, session_count: usize,
    csrf: String, lang: Locale) -> String
pub fn render_user_delete_confirm(user: UserSummary,
    csrf: String, lang: Locale) -> String
pub fn render_user_reset_mfa_confirm(user: UserSummary,
    csrf: String, lang: Locale) -> String
pub fn render_user_force_logout_confirm(user: UserSummary, session_count: usize,
    refresh_token_count: usize, csrf: String, lang: Locale) -> String
pub fn render_client_delete_confirm(client: ClientRow, session_count: usize,
    csrf: String, lang: Locale) -> String
pub fn render_client_regen_secret_confirm(client: ClientRow,
    csrf: String, lang: Locale) -> String
```

Each uses the `reversibility-badge` component from RFC 023.

### Step-up integration

The confirmation screen `GET` handlers check `step_up_freshness`:
- Fresh (within 5 min): render confirmation screen directly.
- Stale: redirect to `/me/security/step-up?return_to=<confirm-url>`.

### Remove `confirm()` calls

All six `onsubmit="return confirm('...')"` attributes are removed once
the confirmation routes are wired.

## New `Strings` fields

```
confirm_disable_title / confirm_disable_impact / confirm_disable_reversibility
confirm_delete_title / confirm_delete_impact / confirm_delete_reversibility
confirm_reset_mfa_title / confirm_reset_mfa_impact
confirm_force_logout_title / confirm_force_logout_impact
confirm_regen_secret_title / confirm_regen_secret_impact / confirm_regen_secret_reversibility
confirm_client_delete_title / confirm_client_delete_impact
badge_recoverable / badge_not_recoverable
confirm_cancel_button
```

## Tests

- E2E: POST to a mutation endpoint without `_confirmed=1` returns 400/403.
- E2E: Confirmation screen renders correct target name and session counts.
- E2E: Cancel button returns to the originating list without mutation.
- E2E: step-up freshness check redirects stale sessions before showing confirm.

## Version

Minor bump (new routes, new UI flows, new form field `_confirmed`).
