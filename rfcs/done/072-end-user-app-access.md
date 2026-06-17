# RFC 072 — End-user app-access surface

**Status.** Implemented (v0.60.0)
**Priority.** P1 — largest end-user information gap for an IdP; users had
no way to see or revoke OAuth grants before this RFC.
**Tracks.** UX rethink — end-user surface (audit notes, v0.57.1 session).
**Touches.** `crates/sui-id-store` (migration 0029, user_consent repo),
`crates/sui-id-core` (TokenSet.user_id), `crates/sui-id` (oidc handler,
me_security apps handler, router), `crates/sui-id-web` (MeTab::Apps,
render_me_apps, me_security/apps.rs), `crates/sui-id-i18n`.

## Implementation note (v0.60.0)

### Schema — migration 0029

```sql
ALTER TABLE user_consent ADD COLUMN last_used_at TIMESTAMP;
```

NULL until the first token exchange after this migration. The UI renders
the original `granted_at` date in that case so the display is always
informative.

### Repo additions (`user_consent.rs`)

**`ConsentGrantView`** — struct joining `user_consent` with the client's
display name (from `clients.name`). Fields: `client_id`, `client_name`,
`granted_scopes`, `granted_at`, `last_used_at`.

**`list_for_user(db, user_id)`** — SELECT joining `user_consent` ⋈
`clients` for non-deleted clients; ordered `granted_at DESC`.

**`revoke_with_tokens(db, user_id, client_id)`** — atomic transaction:
`DELETE FROM refresh_tokens WHERE user_id=? AND client_id=?`, then
`DELETE FROM user_consent WHERE user_id=? AND client_id=?`. The
refresh-token deletion is the safety-critical part; without it an app
holding a stolen refresh token continues to mint access tokens after
the user revokes.

**`touch_last_used(db, user_id, client_id, now)`** — UPDATE `last_used_at`
for the grant. Called best-effort after each successful token exchange.

### `TokenSet.user_id: Option<UserId>`

Added to `sui-id-core::tokens::TokenSet`. Populated by `issue_token_set`
(always `Some` for authorization-code and refresh-token grants). Used by
the token endpoint to call `touch_last_used` without a separate DB lookup.

### Token endpoint (`handlers/oidc.rs`)

After building `TokenResponse`, calls `touch_last_used` if `set.user_id`
is `Some`. Errors are swallowed — a failed timestamp update must never
fail the token response.

### `/me/apps` surface

**`MeTab::Apps`** added to the enum; `key()` returns `"apps"`;
`ME_SECURITY_TABS_KEYS` entry `("apps", "/me/apps")` inserted between
Sessions and Language; labels array gains `t.me_tab_apps`.

**`render_me_apps`** at `crates/sui-id-web/src/pages/me_security/apps.rs`.
Shows a card per grant: client name, Granted / Last used dates, scope list
(reusing `.consent-scope-item` from RFC-MI-070), and a Revoke button that
posts to `/me/apps/{client_id}/revoke`. Empty state uses `.callout--info`.
No new CSS tokens.

**`me_apps_get`** and **`me_apps_revoke`** at
`crates/sui-id/src/handlers/me_security/apps.rs`.

Routes registered:
```
GET  /me/apps                       → me_apps_get
POST /me/apps/{client_id}/revoke    → me_apps_revoke
```

### i18n (9 new keys × 3 locales — en/ja/zh)

`me_tab_apps`, `me_apps_title`, `me_apps_intro`, `me_apps_granted_on`,
`me_apps_last_used`, `me_apps_never_used`, `me_apps_revoke_button`,
`me_apps_revoked`, `me_apps_empty`.

### Acceptance criteria (verified)

- [x] Migration 0029 adds `last_used_at`; existing rows have NULL.
- [x] Token endpoint updates `last_used_at` on successful exchange
  (best-effort; errors logged but not propagated).
- [x] `GET /me/apps` lists all grants with scopes and dates.
- [x] `POST /me/apps/{client_id}/revoke` removes the grant and all
  refresh tokens atomically.
- [x] Apps tab visible in the me_security tab strip on every `/me/` page.
- [x] Empty state shown when the user has no grants.
- [x] 9 i18n keys in en/ja/zh.
- [x] `cargo check --workspace` clean; 175/175 library tests pass.
- [x] CI invariants: `text-leaks`=0, `inline-style-bound`=0,
  `css-tokens`=148, `semantic-parity`=36.

---
**Tracks.** UX rethink — end-user surface (see audit notes, v0.57.1 session).
**Touches.** `crates/sui-id-store` (user_consent repo, optional schema
add), `crates/sui-id-core` (consent revocation logic, refresh-token
invalidation), `crates/sui-id` (new `/me/apps` handlers),
`crates/sui-id-web` (new `MeTab` variant + render fn), `crates/sui-id-i18n`.

---

## Background

This product is an OIDC identity provider. When a user authorizes an
OAuth client (via the consent screen introduced in RFC-MI-070), the grant
is stored in `user_consent` and silently honoured on subsequent
authorization requests. There is no UI for the user to:

- See which apps currently hold a consent grant.
- See when each grant was created and last exercised.
- Revoke a grant (i.e., force re-consent on next sign-in and immediately
  invalidate any refresh tokens issued under that grant).

For a system whose entire purpose is identity, this absence is the single
most user-visible gap. Every modern IdP (Google, Microsoft, Apple,
GitHub) exposes this surface; sui-id should too.

## Non-goals

- **Not a per-scope revocation UI.** Revocation is all-or-nothing per app.
  Granting some scopes and not others remains the responsibility of the
  consent screen at sign-in time.
- **Not a notification system.** The page is reactive (user opens it);
  there is no push when an app is granted, no email when access is used.
- **No remote app revocation.** Revoking the grant does not call any
  endpoint on the app. The grant is removed locally and any active
  refresh tokens are invalidated; the app finds out next time it tries
  to refresh.
- **No history of past grants.** Once revoked, a grant is deleted, not
  archived. The audit log retains a record.

## Goal

Add a new self-service surface at `/me/apps` that:

- Lists every active `user_consent` row for the signed-in user.
- For each entry, shows: app display name, granted scopes (with the
  human-readable descriptions from RFC-MI-070), granted date, last-used
  date.
- Provides a per-entry revoke action that:
  1. Deletes the `user_consent` row.
  2. Revokes all active refresh tokens for `(user_id, client_id)`.
  3. Records an `audit_action = 'consent_revoked'` entry.
  4. Surfaces a flash banner: "Access revoked. The app will need to
     ask for permission again next time you sign in."
- Integrates into the `/me/security/*` tab strip as a new sibling tab
  named **Apps**.

## Design

### Schema migration `0027_user_consent_last_used.sql`

```sql
-- RFC 072: track when each grant was last exercised, so users can see
-- "last used" and audit unused authorizations.
ALTER TABLE user_consent ADD COLUMN last_used_at TIMESTAMP;
-- NULL means "never observed in a token exchange since this column was
-- added," which we render as the original granted_at date.
```

(Note: this migration number conflicts with RFC 071's `0027_users_role`.
Whichever lands first takes 0027; the other shifts to 0028. The order
is RFC 071 → RFC 072.)

### Touchpoint in the token endpoint

In `handlers/oidc.rs` (or wherever the OIDC token endpoint lives),
after a successful authorization-code or refresh-token exchange, update
`user_consent.last_used_at = now()` for the `(user_id, client_id)`
matching the exchange. This is a single `UPDATE` on a primary key — cheap.

If the consent row doesn't exist (because `consent_policy = 'none'`),
no row is updated. This is correct: the apps page will not list that
app, because the user never explicitly consented to it. Sites that want
to show "apps that have ever signed me in regardless of consent" are
out of scope for this RFC (would require a separate audit-derived view).

### Handler: `GET /me/apps`

```rust
pub async fn get_me_apps(
    State(app): State<AppState>,
    session: AuthenticatedSession,
) -> Result<Html, HttpError> {
    let grants = sui_id_store::repos::user_consent::list_for_user(
        &app.db, &session.user_id
    ).await?;

    // For each grant, resolve the client display name and the
    // scope label/description pairs from i18n.
    let view_data: Vec<AppGrantView> = grants.into_iter()
        .map(|g| AppGrantView::from(g, &app.db, locale))
        .collect();

    Ok(render_me_apps(view_data, ...))
}
```

### Handler: `POST /me/apps/{client_id}/revoke`

```rust
pub async fn post_me_apps_revoke(
    State(app): State<AppState>,
    session: AuthenticatedSession,
    Path(client_id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Result<Redirect, HttpError> {
    csrf::verify(&form.csrf, &session)?;

    let user_id = &session.user_id;
    sui_id_core::admin::revoke_consent_and_tokens(
        &app.db, user_id, &client_id
    ).await?;
    audit::log(
        &app.db, audit::Action::ConsentRevoked,
        Some(user_id), Some(&client_id), session.role,
    ).await?;

    Ok(Redirect::to("/me/apps").with_flash(t.me_apps_revoked))
}
```

`revoke_consent_and_tokens` is a new `core::admin` function that, in a
single transaction:

1. `DELETE FROM user_consent WHERE user_id = ? AND client_id = ?`
2. `DELETE FROM refresh_tokens WHERE user_id = ? AND client_id = ?`
3. Records the audit event.

The refresh-token deletion is the safety-critical part. Without it, an
attacker holding a stolen refresh token continues to mint access tokens
even after the user revoked the grant.

Access tokens already in circulation continue to work until they expire
(typically minutes). This is the standard OAuth 2.0 behaviour and
acceptable.

### View: `render_me_apps`

A new page at `crates/sui-id-web/src/pages/me_security/apps.rs` (per
RFC 065's per-screen split convention). Renders a `<ul>` of grants.

For each grant:

```html
<li class="card">
    <div class="grid-2col">
        <div>
            <h3>{client_display_name}</h3>
            <p class="muted">
                {t.me_apps_granted_on}: {granted_at}
                · {t.me_apps_last_used}: {last_used_at_or_never}
            </p>
        </div>
        <form method="post"
              action="/me/apps/{client_id}/revoke"
              class="form-actions">
            <input type="hidden" name="_csrf" value="{csrf}" />
            <button type="submit" class="button danger">
                {t.me_apps_revoke_button}
            </button>
        </form>
    </div>
    <ul class="consent-scope-list">
        <!-- reuses the .consent-scope-item structure from RFC-MI-070 -->
        ...
    </ul>
</li>
```

The `.consent-scope-item` and its `__title` / `__desc` classes are
reused as-is from RFC-MI-070; no new CSS shipped in this RFC.

Empty state: a `.callout` with text "You have not authorized any
applications" and a link explaining what an authorization is.

### Tab strip integration

Extend `MeTab` in `pages/me_security.rs`:

```rust
pub enum MeTab {
    Overview,
    Password,
    Mfa,
    Passkey,
    Sessions,
    Apps,        // ← new
    Language,
}
```

Add `me_tab_apps` i18n key (en/ja/zh). The tab strip helper already
iterates the enum; no further changes required.

### New i18n keys

en / ja / zh:

| Key | en | ja | zh |
|---|---|---|---|
| `me_tab_apps` | Apps | アプリ | 应用 |
| `me_apps_title` | Authorized applications | 認証済みアプリ | 已授权应用 |
| `me_apps_intro` | Apps that can sign in as you. Revoke access at any time. | あなたとしてサインインできるアプリ。いつでも取り消せます。 | 可以代表您登录的应用。您可以随时撤销访问权限。 |
| `me_apps_granted_on` | Granted | 許可日 | 授权时间 |
| `me_apps_last_used` | Last used | 最終使用 | 上次使用 |
| `me_apps_never_used` | Never used | 未使用 | 从未使用 |
| `me_apps_revoke_button` | Revoke access | アクセスを取り消す | 撤销访问 |
| `me_apps_revoked` | Access revoked. The app will need to ask for permission again. | アクセスを取り消しました。アプリは再度許可を求める必要があります。 | 访问已撤销。该应用需要再次请求权限。 |
| `me_apps_empty` | You have not authorized any applications. | 認証済みのアプリはありません。 | 您尚未授权任何应用。 |

## Acceptance criteria

- [ ] Migration 0027 (or 0028 if RFC 071 lands first) adds
  `user_consent.last_used_at`; existing rows have NULL.
- [ ] Token endpoint updates `last_used_at` on successful exchange.
- [ ] `GET /me/apps` lists all grants with scopes, granted/last-used dates.
- [ ] `POST /me/apps/{client_id}/revoke` removes the grant **and** all
  refresh tokens, in a single transaction; logs the audit event.
- [ ] New tab "Apps" present on every `/me/security/*` page; clicking
  takes the user to `/me/apps`.
- [ ] Empty state shown when the user has no grants.
- [ ] Three new i18n keys land in en/ja/zh.
- [ ] CI invariants unchanged.

## Risks

| Risk | Mitigation |
|---|---|
| User revokes a grant they actually wanted, then is confused on next sign-in | Flash banner explicitly states the app will re-prompt; revocation is reversible by re-consenting on next sign-in |
| Race condition: user revokes while a token exchange is in flight | Transaction in `revoke_consent_and_tokens` is atomic; exchange either succeeds (and updates `last_used_at` then is invalidated on next try) or fails — either is safe |
| `last_used_at` shows stale data because the token endpoint forgot to update it | Integration test asserts that after a token exchange the column is updated |
| Stale refresh tokens granted before this RFC exist without a corresponding `user_consent` row | Already handled — refresh tokens are bound to `(user_id, client_id)` independent of the consent table |

## Follow-up RFCs

- **RFC 074 (post-1.0)**: per-scope revocation — let the user say "this
  app can read my email but no longer my profile." Out of scope here.
- **RFC 075 (post-1.0)**: admin view of a user's grants from the user
  detail page, with admin-initiated revoke. Mostly the same code with
  a different handler and authorization check.
