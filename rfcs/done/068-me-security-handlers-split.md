# RFC 068 — `handlers/me_security.rs` split per tab domain

**Status.** Implemented (v0.48.0)
**Priority.** P0 — Phase F (v0.48.0). Bundled with RFC 067 as the
final Phase F buffer release before v1.0-rc1.
**Tracks.** Project spec §8.3 — files over 500 LOC recommend
splitting. `handlers/me_security.rs` is 1099 LOC, the last file in
the codebase still violating the ceiling after Phase F's first two
splits (RFC 065 for `pages.rs`, RFC 066 for `handlers/admin.rs`).
**Touches.** `crates/sui-id/src/handlers/me_security.rs` (becomes
umbrella), new `crates/sui-id/src/handlers/me_security/` directory
with 7 child modules. `crate::handlers::me_security` namespace
unchanged (router.rs and external callers stay).

## Background

`handlers/me_security.rs` grew to 1099 LOC over RFC 055 (consolidate
self-service onto `/me/security/*`, v0.44.0), RFC 056 (recovery
codes count), RFC 057 (language save confirm), RFC 058 (step-up on
self-service routes), RFC 060 (audit-note `"self"` discriminator),
and RFC 063 (passkey rename). It now contains:

- 24 `pub async fn` handlers (the live routes)
- 1 `async fn render_mfa_tab_with_fresh_codes` private helper
- 2 sync `pub async fn` redirect shims (legacy URL compatibility)
- 2 small private helpers (`describe_auth_methods`, `flash_from_query`)
- 9 form-data / query structs (`CsrfOnlyForm`, `RevokeAllOthersForm`,
  `PasswordChangeForm`, `PasskeyRenameForm`, `LanguageGetQuery`,
  `LanguageForm`, `MfaConfirmForm`, `PasskeyRegisterStartForm`,
  `PasskeyRegisterCompleteForm`, `PasskeyDeleteForm`)

The handlers cluster cleanly by the tab they serve, mirroring the
6-tab structure in `crates/sui-id-web/src/pages/me_security/`. The
split below makes the handler-side layout follow the page-side
layout.

## Goal

Split into 7 child modules under
`crates/sui-id/src/handlers/me_security/`, mirroring the 6 self-
service tabs plus a shared `forms.rs`. No file exceeds 500 LOC.
External callers (router.rs) reach handlers through the same
`me_security::handler_name` path because the umbrella `me_security.rs`
re-exports each submodule's `pub` items.

## Design

### Module layout

```
crates/sui-id/src/handlers/
├── me_security.rs              # umbrella: 2 redirects + private helpers
│                               # (describe_auth_methods, flash_from_query) +
│                               # mod declarations + pub use *
└── me_security/
    ├── forms.rs                # 9 form/query structs
    ├── overview.rs             # overview_get + legacy page_get
    ├── mfa.rs                  # mfa_get, mfa_enroll_start,
    │                           # mfa_enroll_confirm, mfa_disable,
    │                           # mfa_regenerate_recovery,
    │                           # render_mfa_tab_with_fresh_codes
    ├── sessions.rs             # sessions_tab_get, revoke_one,
    │                           # revoke_all_others
    ├── passkey.rs              # passkeys_get, passkey_rename_post,
    │                           # passkey_register_start,
    │                           # passkey_register_complete,
    │                           # passkey_delete
    ├── language.rs             # language_get, language_post
    └── password.rs             # password_change_get, password_change_post
```

Rust 2018+ module style: `me_security.rs` is the umbrella, `me_security/`
is the sibling directory. No `mod.rs` anywhere.

### Estimated post-split LOC

| File | Estimated LOC | Within spec? |
|------|--------------:|:---:|
| me_security.rs (umbrella) | ~80 | ✅ |
| me_security/forms.rs | ~80 | ✅ |
| me_security/overview.rs | ~150 | ✅ |
| me_security/mfa.rs | ~250 | ✅ |
| me_security/sessions.rs | ~150 | ✅ |
| me_security/passkey.rs | ~210 | ✅ |
| me_security/language.rs | ~85 | ✅ |
| me_security/password.rs | ~160 | ✅ |

All under 500. No secondary splits needed.

### `page_get` placement

`page_get` is the legacy single-page renderer that pre-dates RFC 040's
tab split. It's still referenced by tests and serves the bare
`/me/security` URL (which now redirects to `/me/security/overview`).
Placement: `overview.rs`, since the overview is what the page now
serves.

### Cross-tab shared helpers

`describe_auth_methods` is used by `overview_get` (lists user's auth
methods) and `sessions_tab_get` (per-session auth method display).
Keep in umbrella `me_security.rs` as `pub(super)` so both submodules
can reach it through `use super::describe_auth_methods;`.

`flash_from_query` is used by every tab GET handler to surface
post-action banners. Keep in umbrella as `pub(super)`.

`security_redirect` (the `/me/security` → `/me/security/overview`
canonical redirect) and `admin_profile_redirect` (legacy
`/admin/profile` → `/me/security/overview` shim for RFC 055
back-compat) stay in the umbrella — they're route-level glue, not
tab handlers.

### Migration approach

Same copy-then-rewire pattern used by RFC 065 and 066:

1. Create `me_security/` directory.
2. Move `forms.rs` first (no dependencies on handler code).
3. Add `me_security.rs` umbrella (replace the existing 1099-LOC
   file). Declare submodules + re-export.
4. For each tab in [language, password, overview, sessions, passkey,
   mfa] — smallest first:
   - Create the corresponding `me_security/{tab}.rs` with handlers.
   - Add per-file `use` block scoped to actual needs.
   - Re-export from `me_security.rs` via `pub use {tab}::*;`.
   - Run `cargo check -p sui-id`.

5. Run `cargo fix --lib -p sui-id --allow-no-vcs` to auto-prune
   unused imports (each submodule inherits a generous `use` block
   that the auto-prune will narrow).

### Build hygiene anticipated

Based on RFC 066's experience:

- 9 `#[derive(Debug, Deserialize, …)]` attributes on form structs
  may detach from their structs during line-range extraction. Need
  to re-attach using the backup file approach.
- ~50 unused-import warnings from monolithic `use` block carrying
  over; auto-pruned by `cargo fix`.
- No latent dead-code or bypass bugs expected — RFC 058 and RFC 060
  already swept this file in Phase D.

### Router unchanged

`handlers.rs` already declares `pub mod me_security;`. The umbrella
`me_security.rs` will re-export each submodule, so route definitions
in `crate::router` like
`.route("/me/security/passkeys/{id}/delete", post(me_security::passkey_delete))`
resolve identically.

## Test plan

1. After each tab migration: `cargo check -p sui-id` PASS.
2. After full migration: `cargo check --workspace --tests` PASS.
3. Unit suite: 228/228 PASS unchanged.
4. Optional: smoke-test the `/me/security/*` routes through curl
   to confirm route → handler resolution. The build proves the
   type-level wiring; smoke test catches any handler-name typos in
   re-exports.

## Rollout

Single release as part of v0.48.0 alongside RFC 067. Pure code-
structural change. No user-visible behavior.

## Risks

- **Mid-extraction derives lost.** Mitigation: same backup-driven
  re-insertion as RFC 066. Detection: `cargo check` reports
  `cannot find attribute serde` and we add the derive back.
- **`describe_auth_methods` visibility**: needs to be `pub(super)`
  so sibling submodules can use it. Detection at build time.
- **page_get legacy compat**: if removed accidentally, the bare
  `/me/security` route 404s. Keep it in `overview.rs`.

## Future work

After v0.48.0, no remaining handler files in the workspace exceed
the 500 LOC ceiling. Phase F closes. v1.0-rc1 is the next planned
tag.
