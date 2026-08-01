# RFC 045 — User disable reason input

**Status.** Implemented (v0.41.0)
**Priority.** Medium
**Touches.** Disable confirm screen, `admin::disable_user`, audit
event note field.

---

## Background

The UI/UX user-management page
(`suiiduiuxdevelopmentsupportv0.29x.pdf`, "user management v0.29.x")
specifies the disable operation as:

> Disable — 復旧可能。**理由入力を推奨**。

Currently `POST /admin/users/{id}/disable` takes only the CSRF + a
confirmation flag (RFC 030). No reason field. The disable event lands
in audit log as `user.disable` with the target ID but no human context.

This RFC adds an optional reason input to the disable confirmation
screen and persists the reason as part of the audit log `note` field.

## Goals

1. Optional reason input on the disable confirm screen (`<textarea>`,
   max 200 chars).
2. Persist the reason in `audit_log.note` for the `user.disable` event.
3. Display the reason on the user detail page when the user is disabled.

## Non-goals

- Mandatory reason. The PDF says "推奨" (recommended), not required.
- Reason history. Only the most recent disable carries a reason; if
  re-enabled and re-disabled, a new reason is captured.
- Free-form i18n. Reasons are stored as-typed; no auto-translation.

## Detailed design

Form change in `render_confirm_disable_user`:

```rust
<div class="field">
    <label for="reason" class="field__label">{t.disable_reason_label}</label>
    <textarea id="reason" name="reason" rows="2" maxlength="200"
              placeholder=t.disable_reason_placeholder></textarea>
    <span class="field__hint">{t.disable_reason_hint}</span>
</div>
```

Handler change in `users_disable_post`:

```rust
let reason = form.reason.trim();
let reason_for_audit = if reason.is_empty() {
    None
} else {
    Some(reason.to_string())  // capped at 200 by textarea maxlength
};
admin_uc::disable_user(&app.db, &app.clock, admin_id, target, reason_for_audit).await?;
```

`admin::disable_user` accepts optional `reason: Option<String>` and passes
it through to `audit_with_note(...)` instead of `audit_ok(...)`.

User detail page reads the most recent `user.disable` audit row for the
target user (where `target_user_id = target` and `action = "user.disable"`),
displays the note if present:

```
[ User: alice (disabled) ]
Disabled by admin on 2026-05-12 14:23
Reason: "Account requested for closure by user via support ticket #4521."
```

## i18n keys

- `disable_reason_label`: "Reason (optional)"
- `disable_reason_placeholder`: e.g. "Why is this user being disabled?"
- `disable_reason_hint`: "Helps future admins understand the action."
- `user_detail_disabled_reason_label`: "Reason for disable"

## Test plan

- E2e: disable with reason → audit log row has the reason in `note`.
- E2e: disable without reason → audit log row has `note = NULL`.
- E2e: 201-char reason gets rejected at form level (browser; server
  truncates as defense in depth).
- E2e: user detail page shows the reason after disable.

## Migration risk

Zero. `audit_log.note` column already exists.

## Estimated effort

~3 hours.
