# RFC 066 — `handlers/admin.rs` split per screen domain

**Status.** Implemented (v0.47.1)
**Priority.** P0 — Phase F (v0.47.0)
**Tracks.** Mirrors RFC 065's pages.rs split on the handler side.
Both files violate the 500-LOC spec; both contribute the bulk of
the project's complexity.
**Touches.** `crates/sui-id/src/handlers/admin.rs` (deleted), new
`crates/sui-id/src/handlers/admin/` directory with 7 child modules,
plus `crates/sui-id/src/handlers.rs` (existing `pub mod admin;`
declaration stays).

## Background

`handlers/admin.rs` is 1531 lines containing 31 handler functions,
4 form-data structs (`DisableForm`, `CsrfOnlyForm`,
`ConfirmedForm`, `ConfirmedReasonForm`), 1 private helper
(`render_qr_svg`), 1 public re-export wrapper (`render_qr_svg_pub`),
and various screen-specific use-case glue.

The handlers cluster naturally by route prefix and concern. The
splits below already exist in the routing (`admin/users/*`,
`admin/clients/*`, etc.); we just need the code to mirror it.

## Goal

Split into per-domain child modules under
`crates/sui-id/src/handlers/admin/`. No file exceeds 500 lines.
External callers (router.rs in particular) reach handlers through
the same `admin::handler_name` path because `mod.rs` re-exports
each submodule's public surface.

## Design

### Module layout

```
crates/sui-id/src/handlers/
├── admin.rs                     # deleted
└── admin/
    ├── mod.rs                   # declares submodules + re-exports
    ├── forms.rs                 # DisableForm, CsrfOnlyForm, ConfirmedForm,
    │                            # ConfirmedReasonForm (shared form structs)
    ├── auth.rs                  # login_get, login_post, logout,
    │                            # mfa_challenge_get/post
    ├── dashboard.rs             # dashboard handler + sparkline data prep
    ├── users.rs                 # 9 users_* handlers
    ├── clients.rs               # 8 clients_* handlers
    ├── signing_keys.rs          # 4 signing_keys_* handlers
    ├── audit.rs                 # audit_get, audit_csv_get
    └── webauthn.rs              # webauthn_auth_start/complete
                                 # (login challenge — distinct from
                                 # /me/security/passkeys/* which live in
                                 # handlers/me_security.rs)
```

### Estimated post-split LOC

| File | Estimated LOC | Within spec? |
|------|--------------:|:---:|
| admin/mod.rs | ~50 | ✅ |
| admin/forms.rs | ~50 | ✅ |
| admin/auth.rs | ~150 | ✅ |
| admin/dashboard.rs | ~120 | ✅ |
| admin/users.rs | ~350 | ✅ |
| admin/clients.rs | ~330 | ✅ |
| admin/signing_keys.rs | ~150 | ✅ |
| admin/audit.rs | ~150 | ✅ |
| admin/webauthn.rs | ~180 | ✅ |

All under the 500 cap. No secondary splits needed.

### `render_qr_svg_pub` placement

This helper was added in v0.44.0 (RFC 055) as a public re-export of
admin.rs's private `render_qr_svg`. It's only called from
`me_security.rs::mfa_enroll_start`. The cleanest home is
`admin/mod.rs` (keeps the existing `crate::handlers::admin::render_qr_svg_pub`
path); the private version stays alongside as a same-module helper.

Alternatively, the QR rendering belongs to neither file —
`sui-id-web` would be the natural home. Deferred to a follow-up
because moving it requires updating the call site too.

### Migration approach

Identical pattern to RFC 065 — copy-then-rewire, one domain at a
time:

1. Create `admin/` directory.
2. Move shared `forms.rs` first (no handler dependencies).
3. Add `admin/mod.rs` declaring submodules + re-exports. Delete
   `admin.rs` and replace its module declaration in `handlers.rs`
   (which already says `pub mod admin;` — no change).
4. For each domain in [audit, dashboard, signing_keys, webauthn,
   auth, users, clients] — smallest first:
   - Create the corresponding `admin/{domain}.rs` with the
     handlers.
   - Re-export from `admin/mod.rs` via `pub use {domain}::*;`.
   - Run `cargo check -p sui-id`.

### What the router.rs change looks like

Nothing changes. Routes like
`.route("/admin/users/{id}/delete", post(admin::users_delete))`
still resolve via `admin::*` re-exports. The path
`admin::users_delete` is now `admin::users::users_delete` under the
hood; `mod.rs`'s `pub use users::*;` reconstructs the flat
interface.

## Test plan

1. After each domain migration: `cargo check -p sui-id` PASS.
2. After full migration: `cargo check --workspace --tests` PASS.
3. Unit suite: 215/215 PASS (no logic changes).
4. Manual: hit each admin route through curl + admin browser to
   confirm route → handler resolution works (the build proves it,
   but a smoke test never hurts).

## Rollout

Single release. Pure code-structural. No public API change. No
user-visible effect.

## Risks

- **Module-private types**: any struct or enum currently
  module-private to `admin.rs` (e.g. `ClientEditQuery`) needs to be
  reachable from the new sibling module. Make it `pub(super)` if
  it's used cross-domain, or leave it `pub(crate)` if it's just
  used within one domain.
- **Compile time**: nine modules instead of one means more parallel
  compilation potential, but also more `use` boilerplate. Net
  expected: slightly faster incremental builds, neutral fresh builds.

## Future work

- An analogous split for `me_security.rs` (1099 LOC, also over
  500). Deferred to a follow-up RFC because the domain there is
  less cleanly partitioned than admin (the file mixes mutative
  routes, GET tab views, and shared helpers all closely coupled).
