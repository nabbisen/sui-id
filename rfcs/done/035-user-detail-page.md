# RFC 035 — Admin user detail page

**Status.** Proposed  
**Priority.** Medium. Operators currently cannot inspect a specific user's
sessions, MFA state, or recent audit history from the admin panel.
The design document (P.9) shows a detail view as a separate screen.  
**Source.** UI/UX design document P.9.  
**Touches.** `crates/sui-id-web/src/pages.rs` (new `render_user_detail`),
`crates/sui-id/src/handlers/admin.rs` (new GET handler),
`crates/sui-id/src/router.rs` (new route),
`crates/sui-id-core` (user detail data assembly).

## Screen responsibility (from design P.9)

- **List:** find / see status / navigate to detail
- **Detail:** view sessions, MFA state, last audit events
- **Create:** minimal form
- **Disable / Delete / Reset MFA / Force logout:** step-up + confirm (RFC 030)

The detail screen is read-only. All mutations go through confirmation screens.

## New route

```
GET /admin/users/{user_id}
```

Links from user list rows replace the current edit-form-in-place pattern.

## Data structure

```rust
pub struct UserDetailData {
    pub user:          UserRow,
    pub sessions:      Vec<SessionSummary>,      // active sessions
    pub mfa_totp:      bool,
    pub passkey_count: usize,
    pub recent_audit:  Vec<AuditLogEntryDto>,    // last 20 events for this user
    pub lang:          Locale,
    pub csrf_token:    String,
}
```

## Screen layout

```
← Back to user list

[Alice]  alice@example.com  [Active] [Admin]

── Authentication ──────────────────────────────
  TOTP: enabled
  Passkeys: 2 registered
  [Manage MFA →]  [Reset MFA]  [Force logout]

── Active sessions ─────────────────────────────
  Started        Expires       Factors   
  12 May 14:07   12 May 22:07  password + TOTP   [Revoke]
  ...

── Recent activity ─────────────────────────────
  [Audit table: last 20 rows for this user]
```

The "Reset MFA" and "Force logout" buttons are links to the confirmation
screens from RFC 030 (step-up + confirm pattern).

The "Manage MFA" link navigates to the admin MFA management flow
(out of scope for this RFC; links to profile for now).

## Tests

- E2E: Navigating to `/admin/users/{id}` shows correct session count.
- E2E: An unknown user ID returns 404.

## Version

Minor bump (new route and screen).
