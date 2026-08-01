# RFC 041 — HIBP enforcement consistency across all password entrypoints

**Status.** Implemented (v0.40.0)
**Priority.** P0 (security correctness)
**Tracks.** v0.40.0
**Touches.** `crates/sui-id-core/src/admin.rs` (signature change to
`create_user`), `crates/sui-id/src/handlers/admin.rs` (pass HIBP
client/mode), settings UI (`/admin/settings/authentication` to surface
HIBP mode), e2e tests.

---

## Background

The UI/UX checklist (`suiiduiuxdevelopmentsupportv0.29x.pdf`,
"HIBP 適用") explicitly calls out the need to lock down where and how
HIBP is applied. The deep-research report further specifies:

> setup / self change / admin reset / forgot-reset で
> mode=off|warn|block をテーブルテスト化。

Current coverage:

| Password entrypoint | HIBP applied? | Source |
|---|---|---|
| Setup wizard (initial admin) | ✅ | `crates/sui-id/src/handlers/setup.rs` |
| Self-service password change (`/me/security/password`) | ✅ | `crates/sui-id-core/src/me_security.rs:95` |
| Forgot-password redemption | ✅ | `crates/sui-id-core/src/forgot_password.rs:309` |
| Admin reset user password | ✅ | `crates/sui-id-core/src/admin.rs:281` |
| **Admin create user** | **❌ NOT APPLIED** | `crates/sui-id-core/src/admin.rs::create_user` |

The hole is small but real: an admin creating a new user with a
known-pwned password bypasses the HIBP policy that everyone else has
to respect. If the deployment runs `hibp_mode = block`, this is a
policy-by-policy inconsistency that an auditor would flag.

Additionally, the HIBP mode value is currently editable **only via
TOML**. The settings page reads it but does not let the operator
change it.

## Goals

1. Apply HIBP to `admin::create_user` with the same semantics as
   `admin::reset_user_password` (mode-driven off/warn/block).
2. Surface the HIBP mode in `/admin/settings/authentication` as an
   editable form field.
3. Add e2e regression tests for the four password entrypoints + admin
   create, all combinations of `mode=off|warn|block`.

## Non-goals

- Changing HIBP semantics (k-anonymity, fail-open in block, etc).
- Adding a `warn` toast UI to the admin create flow (admin is making
  the decision; a simple flash message is enough).

---

## Detailed design

### 1. `admin::create_user` signature change

Current:

```rust
// crates/sui-id-core/src/admin.rs
pub async fn create_user(
    db: &Database,
    clock: &SharedClock,
    actor: UserId,
    spec: CreateUserSpec<'_>,
) -> CoreResult<UserRow> {
    require_admin(db, actor).await?;
    check_password_policy(spec.password)?;
    // ... no HIBP check ...
}
```

New:

```rust
pub async fn create_user(
    db: &Database,
    clock: &SharedClock,
    hibp_client: Option<&dyn HibpClient>,
    hibp_mode: HibpMode,
    actor: UserId,
    spec: CreateUserSpec<'_>,
) -> CoreResult<UserRow> {
    require_admin(db, actor).await?;
    check_password_policy(spec.password)?;

    // RFC 041: enforce HIBP consistently with admin::reset_user_password.
    let _hibp_warned = matches!(
        hibp::enforce_hibp(hibp_mode, hibp_client, spec.password).await,
        HibpEnforcement::Blocked { .. } | HibpEnforcement::Warned { .. }
    );
    // If Blocked, enforce_hibp has already returned an Err; we never reach here.
    // If Warned, we proceed (the admin made the choice) but emit a `_warned` audit
    // suffix below (optional, see audit section).

    // ... rest unchanged ...
}
```

The pattern mirrors `reset_user_password` exactly: `enforce_hibp`
returns `Err(...)` on Blocked, `Ok(Warned { .. })` on warn mode +
known-pwned, `Ok(Clean)` otherwise. The handler passes `hibp_client`
from `app.hibp_client` and `hibp_mode` from `app.config.hibp_mode`.

### 2. Handler call-site update

`crates/sui-id/src/handlers/admin.rs::users_new_post`:

```rust
let row = admin_uc::create_user(
    &app.db,
    &app.clock,
    app.hibp_client.as_deref(),          // NEW
    app.config.hibp_mode(),              // NEW
    admin_id,
    spec,
).await.map_err(HttpError::html)?;
```

### 3. Settings UI for HIBP mode

`/admin/settings/authentication` currently displays the HIBP mode but
does not provide a `<select>` to edit it. Add:

```rust
// pages.rs — render_settings_authentication
<div class="field">
    <label for="s-hibp" class="field__label">{t.settings_auth_hibp_label}</label>
    <select id="s-hibp" name="hibp_mode">
        <option value="off"   selected=move || hibp_mode == "off">{t.settings_auth_hibp_off}</option>
        <option value="warn"  selected=move || hibp_mode == "warn">{t.settings_auth_hibp_warn}</option>
        <option value="block" selected=move || hibp_mode == "block">{t.settings_auth_hibp_block}</option>
    </select>
    <span class="field__hint">{t.settings_auth_hibp_hint}</span>
</div>
```

Backed by:

```rust
// crates/sui-id-store/src/repos/server_settings.rs
pub async fn set_hibp_mode(db: &Database, mode: &str) -> StoreResult<()>
```

New i18n keys (in all three locales):
- `settings_auth_hibp_label`: "HIBP password check mode"
- `settings_auth_hibp_off`: "Off"
- `settings_auth_hibp_warn`: "Warn"
- `settings_auth_hibp_block`: "Block"
- `settings_auth_hibp_hint`: short description of the three modes

### 4. Audit event tagging

When HIBP warns on `admin.create_user`, append `_warned` suffix to the
audit action for that one event, matching the existing pattern in
`reset_user_password`:

```rust
let action = if hibp_warned {
    "user.create_warned_hibp"
} else {
    "user.create"
};
audit_ok(db, actor, action, Some(new_user.id.to_string())).await;
```

This makes the policy-warn surface visible in audit log filtering.

---

## Test plan

### Unit
- `admin::create_user` with `hibp_mode = block` and a known-pwned
  password (mock client) returns Err.
- `admin::create_user` with `hibp_mode = warn` and a known-pwned
  password returns Ok and emits `user.create_warned_hibp`.

### E2e (`tests/e2e/rfc041_hibp_consistency.rs`)

Table-test across 5 entrypoints × 3 modes:

| Entrypoint | mode=off | mode=warn | mode=block (pwned) | mode=block (clean) |
|---|---|---|---|---|
| Setup admin | succeeds | succeeds + warn | fails 400 | succeeds |
| Admin create user | succeeds | succeeds + audit suffix | fails 400 | succeeds |
| Admin reset password | succeeds | succeeds + audit suffix | fails 400 | succeeds |
| Self password change | succeeds | succeeds + warn flash | fails 400 | succeeds |
| Forgot-password redeem | succeeds | succeeds + warn flash | fails 400 | succeeds |

Use the existing `MockHibpClient` (in `crates/sui-id-core/src/hibp.rs::tests`)
with a hardcoded pwned set including `"password123"`.

### Existing tests not to break

The existing e2e for `admin_user.rs` calls `create_user` indirectly
via `POST /admin/users`. Those tests use `mode=off` by default, so
they continue passing. Verify by running the full suite.

---

## Migration risk

- **No schema change.** Pure logic + UI surface.
- The handler signature change is internal to `crates/sui-id` — no
  external API change.
- The HIBP edit UI defaults to the existing stored mode; no operator
  action required to retain current behaviour.

## Estimated effort

- Signature change + handler call-site: 1 hour
- Settings UI: 1.5 hours
- i18n keys (3 locales): 30 minutes
- E2e tests: 2–3 hours

**Total: ~5–6 hours.**

## Version impact

Minor bump (changes public API surface of `sui-id-core`).
