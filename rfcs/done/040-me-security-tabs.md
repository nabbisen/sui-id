# RFC 040 — /me/security tabbed structure

**Status.** Implemented
**Priority.** P0 (highest). The single largest PDF-spec compliance gap.
**Tracks.** v0.40.0 (planned release).
**Touches.** New routes, new render functions, new struct hierarchy in
`sui-id-web`, two new repository functions, **one new migration (0026)**
for `users.preferred_lang` indexing.

---

## Background

The UI/UX design document
([`suiiduiuxdevelopmentsupportv0.29x.pdf`](https://example.com/), "self-service v0.29.x")
specifies `/me/security` as a tabbed surface with five responsibilities:

| Tab | Responsibility |
|---|---|
| 概要 (Overview) | MFA status, passkey count, recent security activity |
| MFA | TOTP enroll/disable, recovery codes regeneration |
| Passkey | Multi-register with nicknames, delete, HTTPS/localhost prerequisite |
| Sessions | Current device, last used, individual revoke, revoke all others |
| Language | User self-sets JA/EN/Zh; falls back to cookie / Accept-Language |

The current implementation collapses all of these into a single
`render_me_security` page. The page works, but it violates the
PDF principle "一画面一責務 / one screen one responsibility" — and it
makes it impossible for an end-user to set their own `preferred_lang`
(currently only admins can set this for themselves via `/admin/profile`).

## Goals

1. Split `/me/security` into five tabbed sub-pages, each with one
   responsibility.
2. Add user-facing language preference setting.
3. Surface passkey nicknames in the UI (the DB column exists since 0004).
4. Maintain backwards compatibility: a `GET /me/security` redirect to
   `/me/security/overview` so existing bookmarks keep working.
5. Reuse the existing `MeSecurityData` fields where possible; introduce
   per-tab data structs only where the existing struct does not cover
   the new content.

## Non-goals

- Changing the actual underlying operations (revoke, MFA enroll, etc.)
  — only restructuring the surface.
- Self-service profile editing beyond language (display name, email
  edits stay deferred).
- WebAuthn enrollment flow restructure — only adding nickname
  display/edit on the existing list.

---

## Data model

### Migration 0026

Add an index to `users.preferred_lang` for the new "users who have a
preferred language set" query patterns we'll need on the Language tab.
This is a read-only optimisation; no schema change beyond the index.

```sql
-- Migration 0026 — index on users.preferred_lang for /me/security/language
CREATE INDEX IF NOT EXISTS idx_users_preferred_lang
  ON users(preferred_lang)
  WHERE preferred_lang IS NOT NULL;
```

### Repository additions

#### `users::set_preferred_lang`
Already exists (used by admin profile). Reused as-is.

```rust
// crates/sui-id-store/src/repos/users.rs (existing)
pub async fn set_preferred_lang(
    db: &Database,
    user_id: UserId,
    lang: Option<String>,
) -> StoreResult<()>
```

#### `user_webauthn_credentials::update_nickname` (NEW)

```rust
// crates/sui-id-store/src/repos/user_webauthn_credentials.rs (new)
pub async fn update_nickname(
    db: &Database,
    credential_id: WebauthnCredentialId,
    user_id: UserId,
    new_nickname: &str,
) -> StoreResult<()> {
    db.with_conn(move |conn| {
        conn.execute(
            "UPDATE user_webauthn_credentials \
             SET nickname = ?1 \
             WHERE id = ?2 AND user_id = ?3",
            rusqlite::params![new_nickname, credential_id.to_string(), user_id.to_string()],
        )?;
        Ok(())
    }).await
}
```

The `user_id` predicate ensures a user can only rename **their own**
credentials — never another user's.

### `sui-id-web` data structs

The current `MeSecurityData` has too many fields for the new layout.
Split into per-tab structs with a common `MeShellData` for the tab nav.

```rust
// crates/sui-id-web/src/pages.rs

/// Shared "frame" — every tab needs this to render the tab navigation
/// and the user identity strip at the top.
pub struct MeShellData {
    pub username: String,
    pub is_admin: bool,
    pub active_tab: MeTab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeTab {
    Overview,
    Mfa,
    Passkey,
    Sessions,
    Language,
}

/// Overview tab — high-level status + recent activity.
pub struct MeOverviewData {
    pub shell: MeShellData,
    pub totp_enabled: bool,
    pub passkey_count: usize,
    pub active_session_count: usize,
    pub recent_events: Vec<MeAuditEntry>,
    pub csrf_token: String,
}

/// MFA tab — TOTP enrollment status + recovery codes regeneration.
pub struct MeMfaData {
    pub shell: MeShellData,
    pub totp_enabled: bool,
    pub recovery_codes_remaining: usize,
    pub csrf_token: String,
}

/// Passkey tab — list with nicknames + register button.
pub struct MePasskeyData {
    pub shell: MeShellData,
    pub passkeys: Vec<PasskeyDescriptor>,  // existing struct, reused
    /// True if the request origin is https or localhost. Drives the
    /// "WebAuthn requires HTTPS or localhost" warning banner.
    pub origin_eligible: bool,
    pub csrf_token: String,
}

/// Sessions tab — list with per-row revoke + revoke-all-others.
pub struct MeSessionsData {
    pub shell: MeShellData,
    pub current_session_id: String,
    pub sessions: Vec<MeSessionDescriptor>,  // existing
    pub csrf_token: String,
}

/// Language tab — self-selected preferred locale.
pub struct MeLanguageData {
    pub shell: MeShellData,
    /// The user's currently set preference, or None for "system default".
    pub current_preferred_lang: Option<String>,
    /// All locales the server supports (from Locale::ALL).
    pub available_locales: Vec<(String, String)>,  // (tag, native_name)
    pub csrf_token: String,
}
```

### Existing `MeSecurityData` deprecation

Mark the existing single-page `MeSecurityData` as `#[deprecated]` but
keep it compiling for backwards compatibility through v0.40.x. Remove
in v0.41.0.

---

## Routes

### New (added)

```
GET  /me/security/overview      — render_me_overview
GET  /me/security/mfa           — render_me_mfa
POST /me/security/mfa/recovery-codes/regenerate
GET  /me/security/passkeys      — render_me_passkey
POST /me/security/passkeys/{id}/rename     — rename_passkey
GET  /me/security/sessions      — render_me_sessions
GET  /me/security/language      — render_me_language
POST /me/security/language      — set_my_preferred_lang
```

### Modified (redirect for back-compat)

```
GET  /me/security  → 302 redirect to /me/security/overview
```

### Existing (unchanged)

```
POST /me/security/sessions/{id}/revoke
POST /me/security/sessions/revoke-all-others
POST /me/security/password
GET  /me/security/step-up
GET  /me/security/step-up/webauthn/start
POST /me/security/step-up/webauthn/finish
```

---

## Handler design

### Shared helper

```rust
// crates/sui-id/src/handlers/me_security.rs

/// Resolve the locale, fetch the user row, and build the MeShellData
/// frame. Called by every tab handler before its specific logic.
async fn build_shell(
    app: &AppState,
    user_id: UserId,
    active_tab: MeTab,
) -> Result<(MeShellData, Locale), HttpError> {
    let user = users::get(&app.db, user_id).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let lang = resolve_user_locale(app, user_id).await;
    Ok((MeShellData {
        username: user.username,
        is_admin: user.is_admin,
        active_tab,
    }, lang))
}
```

### Per-tab GET handler skeleton (e.g. Overview)

```rust
pub async fn overview_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    ctx: SessionContext,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let (shell, lang) = build_shell(&app, user_id, MeTab::Overview).await?;

    let totp_enabled = user_totp::get(&app.db, user_id).await
        .ok().flatten().map(|r| r.enabled).unwrap_or(false);
    let passkey_count = user_webauthn_credentials::count_for_user(
        &app.db, user_id
    ).await.unwrap_or(0);
    let active_session_count = sessions::list_active_for_user(&app.db, user_id)
        .await.map(|v| v.len()).unwrap_or(0);
    let recent_events = audit::recent_for_user(&app.db, user_id, 10).await
        .unwrap_or_default()
        .into_iter()
        .map(|r| MeAuditEntry { at: r.at, action: r.action, result: r.result, note: r.note })
        .collect();

    let token = crate::csrf::ensure_token(&jar);
    let resp = Html(render_me_overview(
        MeOverviewData {
            shell, totp_enabled, passkey_count, active_session_count,
            recent_events, csrf_token: token.clone(),
        },
        app.is_dev_mode, lang,
    )).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}
```

### Language POST handler

```rust
#[derive(serde::Deserialize)]
pub struct LanguageForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    /// Either "ja" / "en" / "zh" / "" (= clear preference).
    pub locale: String,
}

pub async fn language_post(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Form(form): Form<LanguageForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;

    let new = if form.locale.is_empty() {
        None
    } else {
        // Validate against the allow-list to prevent garbage in the DB.
        match Locale::parse(&form.locale) {
            Some(loc) => Some(loc.tag().to_string()),
            None => return Err(HttpError::html(CoreError::BadRequest(
                "unsupported locale".into()
            ))),
        }
    };
    users::set_preferred_lang(&app.db, user_id, new).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    Ok(axum::response::Redirect::to("/me/security/language").into_response())
}
```

### Passkey rename POST handler

```rust
#[derive(serde::Deserialize)]
pub struct PasskeyRenameForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    pub nickname: String,
}

pub async fn passkey_rename_post(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    Path(cred_id): Path<String>,
    Form(form): Form<PasskeyRenameForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;
    let new_name = form.nickname.trim();
    if new_name.is_empty() || new_name.len() > 64 {
        return Err(HttpError::html(CoreError::BadRequest(
            "nickname must be 1–64 characters".into()
        )));
    }
    let cred_id = WebauthnCredentialId::from_str(&cred_id)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid credential id".into())))?;

    user_webauthn_credentials::update_nickname(&app.db, cred_id, user_id, new_name).await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    audit_ok(&app.db, user_id, "webauthn.nickname.updated",
             Some(cred_id.to_string())).await;

    Ok(axum::response::Redirect::to("/me/security/passkeys").into_response())
}
```

---

## UI/UX design — tab navigation

Render at the top of every `/me/security/*` page, similar to the existing
`settings_tabs` helper in `pages.rs`:

```rust
fn me_security_tabs(active: MeTab, lang: Locale) -> impl IntoView {
    let t = lang.strings();
    let items = [
        (MeTab::Overview, t.me_tab_overview, "/me/security/overview"),
        (MeTab::Mfa,      t.me_tab_mfa,      "/me/security/mfa"),
        (MeTab::Passkey,  t.me_tab_passkey,  "/me/security/passkeys"),
        (MeTab::Sessions, t.me_tab_sessions, "/me/security/sessions"),
        (MeTab::Language, t.me_tab_language, "/me/security/language"),
    ];
    // ... same pattern as settings_tabs ...
}
```

### Passkey tab content

```
[ Tab navigation ]

Passkeys

⚠ WebAuthn requires HTTPS or localhost.   ← shown when !origin_eligible

┌─────────────────────────────────────────┐
│ YubiKey 5C                              │
│ Created 2024-01-15  Last used 2024-12-01│
│ [Rename] [Delete]                       │
├─────────────────────────────────────────┤
│ MacBook Touch ID                        │
│ Created 2024-03-20  Last used yesterday │
│ [Rename] [Delete]                       │
└─────────────────────────────────────────┘

[ + Add a passkey ]
```

### Language tab content

```
[ Tab navigation ]

Display language

  ○ Use system default  (Cookie / Accept-Language / server default)
  ● 日本語
  ○ English
  ○ 中文

[ Save ]
```

---

## i18n keys

Add to `Strings`:

```rust
// /me/security tab labels
pub me_tab_overview: &'static str,
pub me_tab_mfa: &'static str,
pub me_tab_passkey: &'static str,
pub me_tab_sessions: &'static str,
pub me_tab_language: &'static str,

// Overview tab
pub me_overview_section_status: &'static str,
pub me_overview_section_activity: &'static str,

// Passkey tab
pub me_passkey_origin_warning: &'static str,  // "WebAuthn requires HTTPS or localhost"
pub me_passkey_section_title: &'static str,
pub me_passkey_button_rename: &'static str,
pub me_passkey_rename_dialog_title: &'static str,
pub me_passkey_nickname_placeholder: &'static str,

// Sessions tab — reuse existing me_security_sessions_* keys

// Language tab
pub me_language_title: &'static str,
pub me_language_lede: &'static str,
pub me_language_use_default: &'static str,
pub me_language_button_save: &'static str,
pub me_language_saved_flash: &'static str,
```

All three locales (ja/en/zh) must populate every new key — the compile-time
exhaustiveness check guarantees this.

---

## Test plan

### Unit
- `users::set_preferred_lang` round-trip (already covered)
- `user_webauthn_credentials::update_nickname` with valid + invalid user_id
  (security: user can't rename someone else's credential)

### E2e (`tests/e2e/rfc040_me_security_tabs.rs`)
1. `GET /me/security` redirects to `/me/security/overview`.
2. Each tab GET returns 200 with the expected tab marked active.
3. `POST /me/security/language` with `locale=en` updates the row and
   subsequent GETs render in English.
4. `POST /me/security/language` with `locale=invalid` returns 400.
5. `POST /me/security/passkeys/{id}/rename` with a foreign user's
   credential id returns no effect (the credential is not renamed).
6. `POST /me/security/passkeys/{id}/rename` with empty nickname returns 400.

---

## Migration risk

- The redirect from `/me/security` to `/me/security/overview` is harmless.
- The new `update_nickname` repo function is additive (no schema change).
- Migration 0026 is a `CREATE INDEX IF NOT EXISTS` — idempotent, fast,
  zero risk on existing databases.
- The deprecated `render_me_security` keeps existing tests passing.

---

## Estimated effort

- Migration 0026: 5 minutes
- Repository function: 30 minutes
- Five tab render functions + handler stubs: 4–6 hours
- i18n keys for three locales: 1 hour
- E2e tests: 2 hours
- Documentation update (`docs/src/guides/operators.md` / a new user guide): 1 hour

**Total: ~8–10 hours of focused work.**

## Version impact

Minor version bump (new routes + new migration + new public API surface
in `sui-id-web`).
