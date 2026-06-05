# RFC 029 — Admin panel i18n completion

**Status.** Proposed  
**Priority.** Medium-High. Every admin operator who uses En or Zh currently
sees Japanese hardcoded strings throughout the admin panel. This is the
broadest i18n gap remaining after RFC 002.  
**Source.** UI/UX design document P.14 — "未完了: admin dashboard / users /
clients / signing keys / audit log / settings Auth-Logs-Email-Other /
404-500-rate-limited / mail templates".  
**Touches.** `crates/sui-id-web/src/pages.rs` (all admin render functions),
`crates/sui-id-i18n/src/strings.rs` (new fields), `ja.rs`, `en.rs`, `zh.rs`.

## Problem

The following render functions lack a `lang: Locale` parameter and contain
hardcoded Japanese strings:

| Function | Hardcoded strings (sample) |
|---|---|
| `render_dashboard` | "ダッシュボード", "ユーザー", "クライアント", "統計サマリ" |
| `render_users` | "ユーザー一覧" |
| `render_clients` | "クライアント管理", "クライアント Secret は今だけ…" |
| `render_audit` | "監査ログ" |
| `render_signing_keys` | "署名キー", "JWT 署名用…", "ローテーション…" |
| `render_error` | Uses `t.error_*` keys but `lang` is not passed |
| Settings tabs (Auth/Logs/Email/Other) | Mixed — titles translated but body labels hardcoded |

## Changes

### 1. New `Strings` fields

Add translation keys for all hardcoded admin strings. Affected groups:

- `dashboard_title`, `dashboard_lede`, `dashboard_stat_users`,
  `dashboard_stat_clients`, `dashboard_stat_sessions`,
  `dashboard_stat_service_status`, `dashboard_activity_title`
- `users_title`, `users_lede`, `users_create_title`,
  `users_table_th_username`, `users_table_th_display`,
  `users_table_th_status`, `users_table_th_mfa`, `users_table_th_created`
- `clients_title`, `clients_lede`, `clients_create_title`,
  `clients_secret_once_banner`,
  `clients_table_th_name`, `clients_table_th_id`,
  `clients_table_th_kind`, `clients_table_th_scopes`,
  `clients_table_th_logout`, `clients_table_th_status`
- `signing_keys_title`, `signing_keys_lede`,
  `signing_keys_rotate_button`, `signing_keys_rotate_warning`,
  `signing_keys_th_id`, `signing_keys_th_algorithm`,
  `signing_keys_th_status`, `signing_keys_th_created`, `signing_keys_th_retired`
- `audit_filter_label`, `audit_filter_placeholder`,
  `audit_export_button`, `audit_copy_row_id`
- Settings body labels (Auth, Logs, Email, Advanced tabs)

### 2. Add `lang` parameter to all affected functions

```rust
pub fn render_dashboard(data: DashboardData, flash: Option<Flash>, lang: Locale) -> String
pub fn render_users(..., lang: Locale) -> String
pub fn render_clients(..., lang: Locale) -> String
pub fn render_audit(..., lang: Locale) -> String
pub fn render_signing_keys(..., lang: Locale) -> String
pub fn render_error(title: String, message: String, request_id: String, lang: Locale) -> String
```

All callers in `handlers/admin.rs` pass `app.config.server.default_lang`
(or the resolved user locale when available).

### 3. Replace hardcoded strings with `t.` references

Mechanical substitution; no logic change.

## Tests

The `each_locale_has_strings` test in `sui-id-i18n` already iterates
`Locale::ALL` and asserts every field is non-empty. Adding fields triggers
a compile failure in all locale files until translated — the completeness
guarantee is automatic.

## Version

Patch bump (no schema change, no API break at the HTTP level).
