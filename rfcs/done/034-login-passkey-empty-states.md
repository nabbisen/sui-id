# RFC 034 — Login passkey primary button + admin UI empty states

**Status.** Implemented (v0.36.0)
**Priority.** Medium. Two small but visible gaps from the design document.  
**Source.** UI/UX design document P.6 (passkey login), P.17 (empty/state copy).  
**Touches.** `crates/sui-id-web/src/pages.rs` (login form, list empty states),
`crates/sui-id-i18n/src/strings.rs` (empty state copy).

## Part A — Passkey as primary login option

### Problem

The design document shows passkey as a first-class login option on the
main login screen (not just as an alternative during MFA challenge).
Current login form has only username + password + forgot-password link.

### Fix

When the server has at least one WebAuthn credential registered (which it
does once any user has enrolled), show a "Sign in with passkey" button
**above** the password form (or separated by an `<hr>`):

```
[Sign in with passkey]
────────────────────
Username
Password
[Sign in]
```

The passkey button POST to `/admin/login/webauthn/start` (already exists).
The password form remains unchanged.

The button is always shown on the login page. Users without a passkey simply
cannot complete the WebAuthn ceremony — the browser will report no credential.
This matches how most modern IdPs handle it.

### `render_login` signature change

```rust
pub fn render_login(
    flash: Option<Flash>,
    next: Option<String>,
    lang: Locale,
    show_passkey_option: bool,  // NEW
) -> String
```

`show_passkey_option` is true when `webauthn_credentials::any_exist(db)`.

## Part B — Empty states for admin lists

### Problem

When user list, client list, or signing-key list is empty, the page
renders an empty table body with no guidance. The design requires
actionable empty-state text.

### Fix

Each list render function checks `items.is_empty()` and renders a
descriptive empty state instead of an empty table:

**Users (empty):**
> "No users registered yet. Create the first user above."

**Clients (empty):**
> "No OIDC clients registered yet. Use the form above to register one."

**Signing keys (empty):**
> "No signing keys. Click 'Rotate signing key' to generate the first key."

### New `Strings` fields

```
users_empty / clients_empty / signing_keys_empty
login_passkey_primary_button   (distinct from mfa_challenge_passkey_button)
```

## Part C — Settings tab "Other" → "Advanced"

The `settings_tab_advanced` i18n key was added in RFC 002 but
`render_settings_other` still uses the hardcoded string `"その他"` in
the tab selector. Fix: update the tab array entry to reference
`t.settings_tab_advanced`.

## Version

Patch bump.
