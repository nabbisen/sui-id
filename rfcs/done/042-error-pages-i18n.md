# RFC 042 — Error and rate-limited page i18n completion

**Status.** Implemented (v0.40.0)
**Priority.** P0 (i18n completeness, last remaining gap)
**Tracks.** v0.40.0
**Touches.** `crates/sui-id-web/src/pages.rs` (render_error), the
error response builder in `crates/sui-id/src/errors.rs`, rate-limiter
middleware response path, new i18n keys.

---

## Background

The UI/UX checklist (`suiiduiuxdevelopmentsupportv0.29x.pdf`,
"a11y + i18n v0.29.x") explicitly lists this as one of the four
remaining i18n gaps:

> 未完了 : 404 / 500 / rate-limited / mail templates

After v0.34.0 (RFC 002), mail templates are per-recipient locale; that
leaves the three HTTP error pages.

Current state:

| Page | Status |
|---|---|
| 404 Not Found | Renders `render_error(404, message)` with English-only message |
| 500 Internal Server Error | Renders English-only "Something went wrong." |
| 429 Too Many Requests | Custom rate-limited page exists but is English-only |
| Generic error fallback | Uses request-id surface, message hardcoded |

The `render_error` function does not take a `lang` parameter and uses
plain strings. This means a Japanese user hitting a 404 sees an English
error page mid-flow — jarring and inconsistent with the rest of the
product's i18n contract.

## Goals

1. Localize all error pages (404 / 500 / 429 / generic) via the
   existing `Locale` resolution chain.
2. Keep the messages **neutral and short** — they must not leak
   information about internal state (per `17.2 アクセシビリティ`).
3. Preserve the request-id display, which is essential for support.

## Non-goals

- Re-designing error pages visually (visual is fine).
- Custom error messages per-route (one neutral message per status
  class is sufficient).
- Localizing every internal `CoreError` variant's `description` field
  (those are operator/log-facing).

---

## Detailed design

### 1. `render_error` signature change

Current:

```rust
pub fn render_error(status: u16, message: &str) -> String {
    // hardcoded layout, English text
}
```

New:

```rust
pub fn render_error(
    status: u16,
    message: &str,         // optional context, may be empty
    request_id: Option<&str>,
    lang: Locale,
) -> String {
    let t = lang.strings();
    let (title, lede) = match status {
        404 => (t.error_404_title, t.error_404_lede),
        429 => (t.error_429_title, t.error_429_lede),
        500..=599 => (t.error_500_title, t.error_500_lede),
        _ => (t.error_generic_title, t.error_generic_lede),
    };
    // ... render shell with title, lede, optional message, request-id ...
}
```

### 2. Error response builder (`HttpError::into_response`)

Resolve the locale at the response edge. The error builder lives in
`crates/sui-id/src/errors.rs`. Today it has access to the request's
`HeaderMap` (for `Accept-Language`) and any session cookie.

Add a helper:

```rust
// crates/sui-id/src/errors.rs
fn resolve_locale_from_headers(
    headers: &HeaderMap,
    server_default: Locale,
) -> Locale {
    // Cookie sui_id_lang > Accept-Language > server default
    if let Some(cookie) = headers.get("cookie") {
        if let Some(v) = parse_cookie(cookie.to_str().unwrap_or(""), "sui_id_lang") {
            if let Some(loc) = Locale::parse(&v) { return loc; }
        }
    }
    if let Some(al) = headers.get(header::ACCEPT_LANGUAGE) {
        if let Some(loc) = parse_accept_language(al.to_str().unwrap_or("")) {
            return loc;
        }
    }
    server_default
}
```

This is the **same resolution chain as authenticated requests minus
`users.preferred_lang`** — by definition we don't have a verified user
when an error page renders (the request may have failed auth). Cookie
+ Accept-Language + server default is the safe subset.

### 3. Rate-limiter middleware

The rate-limiter currently emits a fixed text/html body. Wire it
through the new `render_error`:

```rust
// crates/sui-id/src/middleware/rate_limit.rs
let server_default = state.config.default_lang();
let lang = resolve_locale_from_headers(&headers, server_default);
let html = render_error(429, "", request_id.as_deref(), lang);
Response::builder()
    .status(StatusCode::TOO_MANY_REQUESTS)
    .header("Content-Type", "text/html; charset=utf-8")
    .header("Retry-After", retry_after.to_string())
    .body(html.into())
    .unwrap()
```

### 4. i18n keys

```rust
// In Strings struct
// Error pages (RFC 042)
pub error_404_title: &'static str,
pub error_404_lede: &'static str,
pub error_429_title: &'static str,
pub error_429_lede: &'static str,
pub error_500_title: &'static str,
pub error_500_lede: &'static str,
pub error_generic_title: &'static str,
pub error_generic_lede: &'static str,
pub error_request_id_label: &'static str,
pub error_back_home: &'static str,
```

Sample translations:

**ja.rs**
```rust
error_404_title: "見つかりませんでした",
error_404_lede: "そのページは存在しないか、削除されました。",
error_429_title: "リクエストが多すぎます",
error_429_lede: "しばらく時間をおいてから、もう一度お試しください。",
error_500_title: "サーバーエラー",
error_500_lede: "問題が発生しました。サーバー管理者にお問い合わせください。",
error_generic_title: "エラーが発生しました",
error_generic_lede: "リクエストを処理できませんでした。",
error_request_id_label: "リクエスト ID",
error_back_home: "ホームへ戻る",
```

**en.rs**
```rust
error_404_title: "Not found",
error_404_lede: "That page does not exist or has been removed.",
error_429_title: "Too many requests",
error_429_lede: "Please wait a moment and try again.",
error_500_title: "Server error",
error_500_lede: "Something went wrong. Please contact the server administrator.",
error_generic_title: "An error occurred",
error_generic_lede: "We could not process the request.",
error_request_id_label: "Request ID",
error_back_home: "Back to home",
```

**zh.rs**
```rust
error_404_title: "未找到页面",
error_404_lede: "该页面不存在或已被删除。",
error_429_title: "请求过多",
error_429_lede: "请稍候片刻后再试。",
error_500_title: "服务器错误",
error_500_lede: "发生了问题，请联系服务器管理员。",
error_generic_title: "发生错误",
error_generic_lede: "无法处理该请求。",
error_request_id_label: "请求 ID",
error_back_home: "返回首页",
```

### 5. Page layout

The new render_error layout keeps the existing minimalism:

```
┌────────────────────────────────────────┐
│  sui-id                                │
│                                        │
│  404                                   │
│  Not found                             │
│                                        │
│  That page does not exist or has       │
│  been removed.                         │
│                                        │
│  Request ID: 7f3a1b2c                  │
│                                        │
│  [ Back to home ]                      │
└────────────────────────────────────────┘
```

The `<html lang="...">` attribute is set from the resolved `Locale`,
following the same contract as authenticated pages.

---

## Test plan

### Unit
- `render_error(404, "", None, Locale::Ja)` contains "見つかりませんでした"
- `render_error(429, "", None, Locale::En)` contains "Too many requests"
- `render_error(500, "", Some("abc"), Locale::Zh)` contains "abc" and "服务器错误"

### E2e (`tests/e2e/rfc042_error_pages_i18n.rs`)

1. `GET /nonexistent` with `Accept-Language: ja` → 404 page is Japanese.
2. `GET /nonexistent` with `Accept-Language: en` → 404 page is English.
3. `GET /nonexistent` with `Cookie: sui_id_lang=zh` → 404 page is Chinese.
4. Rate-limit a route (login with rapid fire), confirm 429 response
   has `lang="ja"` and "リクエストが多すぎます" body.
5. Force a 500 via a test-only route (gated by `cfg(test)`), confirm
   localized output.

---

## Migration risk

- **No schema change.**
- `render_error` signature changes — all internal call-sites need updating.
  A `grep -rn 'render_error('` will surface ~6–8 sites in `crates/sui-id`.
- The fallback to server default locale ensures no request ever renders
  with no `Locale` selected.

## Estimated effort

- Signature change + 6–8 call-site updates: 1.5 hours
- i18n keys (3 locales): 1 hour
- Rate-limiter wiring: 1 hour
- E2e tests: 2 hours

**Total: ~5–6 hours.**

## Version impact

Patch bump if no other RFC ships in the same release; minor if bundled
with RFC 040/041. Internal API surface change (`render_error`
signature) but no public consumer beyond `crates/sui-id`.
