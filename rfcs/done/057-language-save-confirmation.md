# RFC 057 — Language save confirmation

**Status.** Implemented (v0.44.0)
**Priority.** P1 — Phase C (v0.44.0)
**Tracks.** HANDOFF §2.4 — "Language tab silently saves, no
confirmation. User can't tell whether the click took effect."
**Touches.** `crates/sui-id/src/handlers/me_security.rs::language_get`
(reads `?saved`), `crates/sui-id-web/src/pages.rs::render_me_language`
(renders success banner), `crates/sui-id-i18n/src/strings.rs` and
locale files (one new key).

## Background

The `/me/security/language` POST handler currently redirects to
`/me/security/language?saved=1` after a successful save:

```rust
Ok(Redirect::to("/me/security/language?saved=1").into_response())
```

But the GET handler ignores the `?saved` query parameter and the
view doesn't render any confirmation. The user clicks "Save,"
the page reloads to the same form with no visible change. Did
the click take? Did the language re-render with the new
preference, given that the new preference might equal the
browser's effective locale anyway? There is no feedback.

## Decision

Use the `?saved=1` query approach over the alternative serialisable
Flash banner. Reasons:

- The `?saved=1` pattern is already in `language_post`'s redirect
  target — RFC 057 just consumes what was already wired.
- Flash banners require session state on the server (or signed
  cookie); query strings are stateless. The lighter-weight
  approach fits a "single click, single ack" use case.
- Other Phase C work (`/admin/profile` consolidation, RFC 055) is
  the broader self-service tightening; introducing a Flash
  framework just for this single ack would be out of scope.

## Design

### Handler

`me_security::language_get` gains a `Query` extractor:

```rust
#[derive(Deserialize)]
struct LangGetQuery {
    saved: Option<u8>,
}

pub async fn language_get(
    state_ext: AppStateExt,
    CurrentUser(user_id): CurrentUser,
    jar: CookieJar,
    crate::handlers::RequestLocale(req_locale): crate::handlers::RequestLocale,
    Query(q): Query<LangGetQuery>,
) -> Result<Response, HttpError> {
    // ... existing logic ...
    let just_saved = q.saved == Some(1);
    let resp = axum::response::Html(sui_id_web::render_me_language(
        sui_id_web::MeLanguageData {
            shell,
            current_preferred_lang,
            csrf_token,
            just_saved,                // new field
        },
        None, app.is_dev_mode, lang,
    )).into_response();
    Ok(resp)
}
```

`Option<u8>` keeps the type narrow — accept `?saved=1`, ignore
anything else. Don't accept `?saved=anything-else` lest a typo
or stale link still trigger the banner.

### View

`render_me_language` checks `just_saved` and conditionally
prepends a success banner above the form, similar to how
existing flash banners render:

```rust
{just_saved.then(|| view! {
    <div class="banner banner--success" role="status">
        {t.me_security_language_saved_banner}
    </div>
})}
```

`role="status"` is the WAI-ARIA recommendation for
non-interrupting success feedback (vs `role="alert"` for
errors).

### MeLanguageData

Add one bool field:

```rust
pub struct MeLanguageData {
    pub shell: MeShellData,
    pub current_preferred_lang: Option<String>,
    pub csrf_token: String,
    pub just_saved: bool,             // new
}
```

### i18n keys

| Field | ja | en | zh |
|-------|----|----|----|
| `me_security_language_saved_banner` | `言語設定を保存しました。` | `Language preference saved.` | `语言偏好已保存。` |

### Banner CSS

The `banner banner--success` class needs to exist or be added.
Inspect `components.rs` — if `banner--warning` and
`banner--danger` exist, add `banner--success` symmetrically with
`var(--success-default)` / `var(--success-subtle)` from the RFC
049 token palette.

## Test plan

1. Unit: `MeLanguageData` constructs with `just_saved: true`
   and `false`; render outputs differ accordingly.
2. E2E: POST `/me/security/language` with a valid locale →
   response is 303/302 to `/me/security/language?saved=1`.
   Follow redirect → response body contains the localised
   success banner.
3. Manual: change language to Chinese → banner reads
   `语言偏好已保存。` (i.e. localised to the **just-saved**
   locale, not the previous locale).

## Rollout

Single release. Backward-compatible — `?saved` was already in
the redirect target but ignored; adding a banner doesn't
change any data contract.

## Future work

If similar single-action acknowledgements appear elsewhere
(MFA enrolled, passkey added, session revoked), consider
generalising the `?saved={action}` pattern into a small
`SavedKind` enum that the view dispatches on. RFC 057 deliberately
solves only the language case to avoid premature generalisation;
the natural extension lives one release ahead, after Phase D
(dangerous-operation contracts) crystallises which actions need
their own confirmation messaging.
