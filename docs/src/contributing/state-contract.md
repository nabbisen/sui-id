# UI state word contract

Every page in sui-id must handle five states. Define the copy for each
before you write the render function — that way the translator and the
designer see a consistent vocabulary, and reviewers can confirm the
contract is met in CI.

## The five states

### 1. empty — nothing here yet

**When to use.** A list or table has zero rows, a filter matches nothing,
or a freshly initialised system has no records.

**Tone.** Calm and informative. Point to the next action where possible.

**CSS class.** `.empty-state`

**Key prefix.** `t.{section}_empty` (e.g. `t.profile_passkeys_empty`,
`t.users_empty`, `t.clients_empty`).

**Pattern.**
```rust
{if items.is_empty() {
    view! { <div class="empty-state">
        <p class="muted">{t.section_empty}</p>
    </div> }.into_any()
} else {
    /* item list */
}}
```

### 2. error — something went wrong

**When to use.**
- HTTP error responses (404 / 429 / 5xx): call `render_error(status, request_id, lang)`.
- Form validation failure: flash banner using `t.{action}_failed_flash`.
- Inline field error: `<span class="field__error">{t.{field}_invalid}</span>`.

**Tone.** Neutral and short. Never expose internal details (stack traces,
SQL errors). Include the request ID when available so operators can
correlate with server logs.

**CSS classes.** `.banner.banner--danger` (flash), `.field__error` (inline).

**Rule.** Never add raw error strings to the UI. Always go through an i18n
key so the message can be translated and reviewed.

### 3. success — action completed

**When to use.** After a POST that the user initiated succeeds.

**Tone.** Confirmatory past-tense. State what changed.

**CSS class.** `.banner.banner--ok`

**Key suffix.** `_flash` (e.g. `t.user_created_flash`,
`t.password_changed_flash`, `t.language_saved_flash`).

**Pattern.**
```rust
session.set_flash(Flash { kind: FlashKind::Info, text: t.user_created_flash.into() });
Redirect::to("/admin/users").into_response()
```

The redirect consumes the flash on the next GET; the secret never persists
beyond a single page render.

### 4. loading — work in progress

**When to use.** Only for client-side async operations (WebAuthn
ceremonies). Server-side SSR renders are never "loading" — they either
succeed or return an error page.

**Tone.** Brief. No spinner emoji.

**Key.** `t.loading` (generic) or `t.webauthn_in_progress`.

### 5. disabled — control exists but cannot be used now

**When to use.** A button or control requires a precondition that is not
yet met (e.g. "Test connection" before SMTP is configured).

**Tone.** One sentence explaining why. If you cannot write that sentence,
the control should not exist on this page.

**CSS.** `<button disabled>` + `<span class="field__hint">` below.

**Key suffix.** `_disabled_hint` (e.g. `t.smtp_test_disabled_hint`).

**Rule.** Every disabled button must have an accompanying hint. No silent
disabled buttons.

---

## Page audit table

Add a row when you ship a new page.

| Page | empty | error | success | loading | disabled |
|---|---|---|---|---|---|
| `/admin/users` | `users_empty` | field errors | `user_created_flash` | — | — |
| `/admin/clients` | `clients_empty` | field errors | `client_created_flash` | — | — |
| `/admin/audit` | — | — | — | — | — |
| `/admin/settings/email` | — | inline fields | `settings_smtp_saved_flash` | — | `smtp_test_disabled_hint` |
| `/me/security/passkeys` | `profile_passkeys_empty` | passkey errors | `profile_passkeys_deleted_flash` | `webauthn_in_progress` | origin warning |
| `/me/security/language` | — | — | `me_language_saved_flash` | — | — |

---

## Quick selection guide

```
New page → define copy for all five states before writing Rust
New button → is it conditionally unavailable? → add _disabled_hint key
New list → always add an _empty key
New write operation → always add a _flash key for success
New error condition → add to render_error or a _failed_flash key
```

See also: [`crates/sui-id-i18n/STATE_WORDS.md`](../../../crates/sui-id-i18n/STATE_WORDS.md)
for the canonical key naming conventions.
