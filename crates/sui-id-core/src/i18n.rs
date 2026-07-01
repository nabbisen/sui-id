//! Locale resolution.
//!
//! Picks one [`Locale`] for the current request out of the four
//! signals we accept, in priority order:
//!
//!   1. `user.preferred_lang` — explicit per-user setting
//!      (`/me/profile`). Takes precedence over everything else
//!      because the user has actively chosen this.
//!   2. Cookie `sui_id_lang` — per-browser override. Lets a
//!      signed-out user (no `user_id`) still pick a language, and
//!      lets a signed-in user override on a different machine
//!      without changing the per-user setting.
//!   3. `Accept-Language` header — the browser's default. The
//!      user has not actively chosen anything; we just match what
//!      they sent. We pick the first locale we recognise (q=
//!      weights are intentionally ignored — the cost of a real
//!      parser outweighs the benefit for a two-locale catalogue).
//!   4. `server_settings.default_lang` — admin-configured
//!      fallback. The server's "this is what we serve to people
//!      who haven't told us anything" setting.
//!   5. Hard-coded `Locale::Ja` if everything else fails. Should
//!      not happen post-migration (server_settings has a default
//!      row), but the type system guarantees we always return a
//!      locale.
//!
//! All five tiers are pure functions of their inputs, so this
//! module has no side effects and is trivially testable.

use crate::errors::CoreResult;
pub use sui_id_i18n::{Locale, STRINGS_EN, STRINGS_JA, Strings, negotiate_from_accept_language};
use sui_id_shared::ids::UserId;
use sui_id_store::Database;
use sui_id_store::repos::{server_settings, users};

/// Inputs to the resolver. Constructing this struct in the HTTP
/// layer keeps the `resolve` function testable without a request
/// object.
pub struct LocaleInputs<'a> {
    /// Authenticated user's id, or `None` for anonymous flows
    /// (login, forgot-password, ...).
    pub user_id: Option<UserId>,
    /// Value of the `sui_id_lang` cookie, if set.
    pub cookie: Option<&'a str>,
    /// Value of the `Accept-Language` HTTP header, if set.
    pub accept_language: Option<&'a str>,
}

/// Walk the resolution chain and return the first match.
///
/// Database access is at most two short reads (one for the user,
/// one for the server-settings singleton). Both are skipped when
/// an earlier tier already matched.
pub async fn resolve(db: &Database, inputs: &LocaleInputs<'_>) -> CoreResult<Locale> {
    // 1. user preference
    if let Some(uid) = inputs.user_id {
        if let Some(row) = users::find_by_id_opt(db, uid).await? {
            if let Some(tag) = row.preferred_lang.as_deref() {
                if let Some(loc) = Locale::parse(tag) {
                    return Ok(loc);
                }
                // Tag in DB doesn't match a locale we know — could
                // be stale after a downgrade. Fall through to the
                // next tier rather than erroring.
            }
        }
    }
    // 2. cookie
    if let Some(c) = inputs.cookie {
        if let Some(loc) = Locale::parse(c) {
            return Ok(loc);
        }
    }
    // 3. Accept-Language
    if let Some(h) = inputs.accept_language {
        if let Some(loc) = negotiate_from_accept_language(h) {
            return Ok(loc);
        }
    }
    // 4. server default
    let row = server_settings::get(db).await?;
    if let Some(loc) = Locale::parse(&row.default_lang) {
        return Ok(loc);
    }
    // 5. final fallback. Migration 0016 inserts a known-good
    // default, so reaching here means the row was tampered with;
    // we return Ja rather than error since UI rendering must not
    // be blocked by a stale config row.
    Ok(Locale::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::password::hash_password;
    use chrono::Utc;
    use sui_id_store::crypto::MasterKey;
    use sui_id_store::models::{CredentialRow, UserRow};

    fn fresh_db() -> Database {
        let key = MasterKey::generate();
        Database::open_in_memory(key).expect("db")
    }

    async fn make_user(db: &Database, preferred_lang: Option<&str>) -> UserId {
        let id = UserId::new();
        let now = Utc::now();
        users::create(
            db,
            &UserRow {
                id,
                username: "alice".into(),
                display_name: None,
                is_admin: false,
                role: if false {
                    sui_id_store::models::Role::Admin
                } else {
                    sui_id_store::models::Role::User
                },
                last_login_at: None,
                is_disabled: false,
                is_deleted: false,
                user_uuid: uuid::Uuid::new_v4(),
                created_at: now,
                updated_at: now,
                failed_login_count: 0,
                locked_until: None,
                email: None,
                preferred_lang: preferred_lang.map(str::to_owned),
                email_normalized: None,
                email_verified_at: None,
            },
        )
        .await
        .expect("user");
        let _ = hash_password("alice-the-tester-password");
        let _ = CredentialRow {
            user_id: id,
            password_hash: String::new(),
            must_change: false,
            updated_at: now,
        };
        id
    }

    #[tokio::test]
    async fn user_preference_wins() {
        let db = fresh_db();
        let uid = make_user(&db, Some("en")).await;
        let loc = resolve(
            &db,
            &LocaleInputs {
                user_id: Some(uid),
                cookie: Some("ja"),
                accept_language: Some("ja"),
            },
        )
        .await
        .expect("resolve");
        assert_eq!(loc, Locale::En);
    }

    #[tokio::test]
    async fn cookie_wins_over_accept_language() {
        let db = fresh_db();
        let loc = resolve(
            &db,
            &LocaleInputs {
                user_id: None,
                cookie: Some("en"),
                accept_language: Some("ja"),
            },
        )
        .await
        .expect("resolve");
        assert_eq!(loc, Locale::En);
    }

    #[tokio::test]
    async fn accept_language_used_when_no_user_or_cookie() {
        let db = fresh_db();
        let loc = resolve(
            &db,
            &LocaleInputs {
                user_id: None,
                cookie: None,
                accept_language: Some("en-US,en;q=0.9"),
            },
        )
        .await
        .expect("resolve");
        assert_eq!(loc, Locale::En);
    }

    #[tokio::test]
    async fn falls_back_to_server_default_when_nothing_matches() {
        let db = fresh_db();
        // Migration default is "ja"; nothing else matches.
        let loc = resolve(
            &db,
            &LocaleInputs {
                user_id: None,
                cookie: None,
                accept_language: Some("xx"), // unknown locale → fall back
            },
        )
        .await
        .expect("resolve");
        assert_eq!(loc, Locale::Ja);
    }

    #[tokio::test]
    async fn user_preference_with_unknown_tag_falls_through() {
        // After a hypothetical downgrade, a row could hold a tag
        // we don't recognise. Resolution should not error; it
        // should fall through to subsequent tiers.
        let db = fresh_db();
        // Use a locale tag that will remain unknown even after zh is added.
        let uid = make_user(&db, Some("xx-UnknownLocale")).await;
        let loc = resolve(
            &db,
            &LocaleInputs {
                user_id: Some(uid),
                cookie: Some("en"),
                accept_language: None,
            },
        )
        .await
        .expect("resolve");
        assert_eq!(loc, Locale::En);
    }
}
