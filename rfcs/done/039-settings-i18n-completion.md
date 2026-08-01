# RFC 039 — Settings UI i18n completion

**Status.** Implemented (v0.39.0)
**Priority.** Medium. 102 hardcoded Japanese strings remain in the six
settings tabs. All are either form labels, descriptions, or help text.
Completing this allows operators using English or Chinese to read all
settings content in their chosen language.  
**Touches.** `crates/sui-id-i18n/src/strings.rs` (+~80 new keys),
`ja.rs`, `en.rs`, `zh.rs`, `crates/sui-id-web/src/pages.rs`
(replace hardcoded strings with `t.` references).

## Affected tabs

| Tab | Hardcoded count | Representative strings |
|---|---|---|
| Email | 25 | SMTP connection labels, TLS mode options, From address fields |
| Security | 23 | Lockout description, CORS labels, header names |
| Authentication | 22 | Password policy labels, MFA factor list, token lifetimes |
| Advanced | 20 | DB path, key file path, record counts, server time |
| Basic | 13 | Default language hint, trusted proxies hint, OIDC endpoint names |
| Logs | 13 | Log filter label, chain description, legacy row note |

## Approach

Group strings by semantic category and add fields to `Strings`. Strings
that are already bilingual (e.g. "PKCE 必須(全 client、全 flow)") should be
split into English only — the translation system handles locale selection.

Technical identifiers (`Ed25519`, `Argon2id`, `PKCE`, `TOTP`, `JWKS`,
etc.) stay as English literals regardless of locale; they are proper nouns
and should not be translated.

## Version

Patch bump.
