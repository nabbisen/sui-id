# RFC 044 — State word contract documentation

**Status.** Implemented (v0.40.0)
**Priority.** P1 (process / consistency, no runtime code change)
**Tracks.** v0.40.0
**Touches.** `docs/src/contributing/state-contract.md` (new),
`crates/sui-id-i18n/STATE_WORDS.md` (new), pages.rs comments
(non-functional).

---

## Background

The UI/UX handoff document
(`suiiduiuxdevelopmentsupportv0.29x.pdf`, "handoff v0.29.x") lists as
the third pillar of the design hand-off:

> 3. 状態文言を先に決める
>    - empty
>    - error
>    - success
>    - loading
>    - disabled

And reinforces this in the checklist:

> 状態文言 : empty/error/success/loading/disabled を画面ごとに先に決める。

Today, sui-id implements these states ad-hoc:

| State | Where it shows up | Consistency |
|---|---|---|
| empty | RFC 034 implemented empty states across major lists | ✅ centralized in `t.empty_*` keys |
| error | render_error + flash banners + form field errors | 🔸 three different patterns |
| success | flash banners only | ✅ via `Flash::Success` |
| loading | rarely (SSR) — but step-up redirects show a loader | 🔸 no contract |
| disabled | buttons, banners, badges | 🔸 no single contract |

The work in RFC 042 (error pages i18n) will tighten the error story.
But there is no project-level document codifying which state words to
use where, in three languages, in a way that future contributors can
reference. This RFC is that document.

This RFC ships **no runtime code change**. It is a contributor-facing
contract.

## Goals

1. Document the canonical phrasing in `Strings` for each of the five
   state words.
2. Define the **selection rule**: when a contributor adds a new page,
   which `t.*` key should they reach for first.
3. Specify the visual layer (CSS class) that accompanies each state.
4. Add a section to the contributing guide that points to this contract.

## Non-goals

- Adding new `t.*` keys for states already covered.
- Rewriting existing copy — only documenting what we already have.
- Changing visual styling.

---

## Detailed design

### The five state words

#### 1. empty — "the system is fine, but there is nothing here"

| | |
|---|---|
| When | Lists with zero rows, filtered tables with no matches, freshly initialised systems |
| Tone | Calm, informative, points to the next action |
| Visual | `.empty-state` container, dim text, no border |
| Existing keys | `t.empty_users`, `t.empty_clients`, `t.empty_audit_log`, `t.empty_passkeys`, `t.empty_sessions`, `t.empty_signing_keys` |
| Example (ja) | "ユーザーがまだ登録されていません。" |
| Example (en) | "No users have been registered yet." |
| Example (zh) | "尚未注册任何用户。" |

**Selection rule.** If you're writing a list / table renderer:

```rust
{if items.is_empty() {
    view! { <div class="empty-state"><p class="muted">{t.empty_FOO}</p></div> }.into_any()
} else {
    /* render items */
}}
```

Add a new `empty_FOO` key only if no existing one fits.

#### 2. error — "something went wrong, here's what"

| | |
|---|---|
| When | HTTP error pages (404 / 429 / 5xx), form validation failures, flash banners after a failed action |
| Tone | Neutral, short, never leaks internal state, includes a request ID if available |
| Visual | `.banner.banner--danger` (flash), `.error-page` (full page), `.field__error` (inline) |
| Existing keys | RFC 042 adds `t.error_404_title`, etc. Flash texts use action-specific keys (e.g. `t.password_change_failed_flash`) |
| Example (ja) | "リクエストが多すぎます。しばらく時間をおいてから、もう一度お試しください。" |
| Example (en) | "Too many requests. Please wait a moment and try again." |
| Example (zh) | "请求过多。请稍候片刻后再试。" |

**Selection rule.**
- HTTP error response → `render_error(status, "", request_id, lang)`.
- Form submit failed → flash banner with `t.X_failed_flash`.
- Inline field error → `<span class="field__error">{t.X_invalid}</span>`.

Never include internal error details (stack traces, SQL errors, etc.) —
those go to `tracing` logs only.

#### 3. success — "the action completed"

| | |
|---|---|
| When | After a write operation that the user initiated |
| Tone | Confirmatory, past-tense, what was done |
| Visual | `.banner.banner--success` (jade accent) |
| Existing keys | Action-specific: `t.user_created_flash`, `t.client_disabled_flash`, `t.password_changed_flash` |
| Example (ja) | "パスワードを変更しました。" |
| Example (en) | "Your password has been changed." |
| Example (zh) | "已修改密码。" |

**Selection rule.** Use `Flash::Success(t.X_ed_flash)` after the redirect:

```rust
session.set_flash(Flash::Success(t.user_created_flash.into()));
Redirect::to("/admin/users").into_response()
```

The past-participle naming (`_changed`, `_created`, `_revoked`) keeps
the keys easy to find via grep.

#### 4. loading — "the system is working"

| | |
|---|---|
| When | Rare in SSR. WebAuthn ceremonies show one (browser handles it). Future async UIs might add more. |
| Tone | Brief, not playful, no spinner emoji |
| Visual | `.loading-indicator` (CSS-only spinner via inline SVG) |
| Existing keys | `t.loading` (generic), `t.webauthn_in_progress` |
| Example (ja) | "処理中…" |
| Example (en) | "Working..." |
| Example (zh) | "处理中…" |

**Selection rule.** Only render loading state for client-side async
operations. Server-side renders are never "loading" — they either
succeed or return error.

#### 5. disabled — "this control exists but cannot be used right now"

| | |
|---|---|
| When | Buttons that need preconditions (e.g. "Send test email" before SMTP is configured), tabs for features that aren't available |
| Tone | Explanatory, points to the precondition |
| Visual | `<button disabled>` (browser styling) + `<span class="field__hint">` below |
| Existing keys | `t.smtp_test_disabled_hint`, `t.mfa_disable_unavailable_hint` |
| Example (ja) | "SMTP 設定後に有効になります。" |
| Example (en) | "Becomes available after SMTP is configured." |
| Example (zh) | "在配置 SMTP 后可用。" |

**Selection rule.** A disabled button must always be accompanied by a
hint explaining why. If you can't write the hint in one sentence, the
button shouldn't be disabled — it should not exist on this page.

---

## Page-by-page state-word audit

A non-exhaustive table to seed new pages. Future contributors update
this table when they add a page.

| Page | empty | error | success | loading | disabled |
|---|---|---|---|---|---|
| `/admin/users` | "No users yet" | "Failed to load users" (network) | "User created" | — | — |
| `/admin/clients` | "No clients yet" | "Failed to load clients" | "Client created" | — | — |
| `/admin/audit` | "No events match the filter" | "Failed to load audit log" | — | — | — |
| `/admin/settings/email` | — | inline field errors | "SMTP settings saved" | — | "Test connection" (disabled when not configured) |
| `/me/security/passkeys` | "No passkeys yet" | "Passkey registration failed" | "Passkey added" | "Waiting for security key" (browser) | "Add a passkey" (disabled when origin not eligible) |

---

## Where this lives

Two files:

1. **`docs/src/contributing/state-contract.md`** — Public-facing
   contributor reference. The bulk of this RFC, formatted as a guide.
   Linked from the contributing guide TOC.

2. **`crates/sui-id-i18n/STATE_WORDS.md`** — Code-adjacent reference
   for translators and i18n maintainers. Lists the canonical
   key prefixes (`empty_`, `*_failed_flash`, `*_changed_flash`,
   `loading`, `*_disabled_hint`) and the translation conventions.

Both files cross-reference each other.

---

## Test plan

This RFC ships documentation only. Verification is:

- The two new docs render without broken links (`mdbook build`).
- A reviewer reads each section and confirms the existing pages
  match the documented contract.
- An audit pass (manual grep) confirms every page in `pages.rs`
  applies the five-state pattern correctly. Mismatches are tracked
  as follow-up issues, not blockers.

---

## Migration risk

Zero. Documentation-only.

## Estimated effort

- Drafting `state-contract.md`: 2 hours
- Drafting `STATE_WORDS.md`: 1 hour
- Cross-references + TOC integration: 30 minutes
- Manual audit pass + follow-up issue creation: 1 hour

**Total: ~4–5 hours.**

## Version impact

Patch bump.
