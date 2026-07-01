//! Self-service security handlers (`/me/security/*`).
//!
//! Split per tab domain (RFC 068, v0.48.0). The original
//! `me_security.rs` was 1099 LOC and over the spec's 500-LOC
//! ceiling. It now exists as this umbrella plus per-tab submodules
//! mirroring the 6-tab page layout in
//! `crates/sui-id-web/src/pages/me_security/`.
//!
//! External callers (routes wired in `crate::router`) still reach
//! handlers through `crate::handlers::me_security::handler_name` —
//! each submodule's `pub` items are flattened into the
//! `me_security` namespace by the `pub use {submodule}::*;`
//! re-exports below.
//!
//! Rust 2018+ module style is used throughout — this file is the
//! umbrella; submodules live in `me_security/` as sibling .rs
//! files. No `mod.rs`.

use axum::response::Redirect;
use axum_extra::extract::cookie::CookieJar;

pub mod forms;
mod overview;
mod mfa;
mod sessions;
mod passkey;
mod language;
mod password;
mod apps;        // RFC 072

pub use forms::*;
pub use overview::*;
pub use mfa::*;
pub use sessions::*;
pub use passkey::*;
pub use language::*;
pub use password::*;
pub use apps::*;

// ---------- umbrella-level redirects ----------

/// GET /me/security → /me/security/overview (canonical redirect).
pub async fn security_redirect() -> Redirect {
    Redirect::to("/me/security/overview")
}

/// GET /admin/profile → /me/security/overview (RFC 055 legacy compat).
///
/// The legacy `/admin/profile` single-page UI was consolidated onto
/// `/me/security/*` in v0.44.0. This stub honours existing
/// bookmarks. `permanent()` emits HTTP 308; for a GET route that's
/// equivalent to 301.
pub async fn admin_profile_redirect() -> Redirect {
    Redirect::permanent("/me/security/overview")
}

// ---------- shared private helpers ----------

/// Format the list of authentication methods used in a session as a
/// short human-readable string. Used by both the overview tab and
/// the sessions tab; `pub(super)` so siblings can `use super::*;`.
pub(super) fn describe_auth_methods(methods: &[sui_id_shared::AuthMethod]) -> String {
    use sui_id_shared::AuthMethod;
    if methods.is_empty() {
        return "—".into();
    }
    let parts: Vec<&str> = methods
        .iter()
        .map(|m| match m {
            AuthMethod::Pwd => "password",
            AuthMethod::Totp => "TOTP",
            AuthMethod::RecoveryCode => "recovery code",
            AuthMethod::Webauthn => "passkey",
            AuthMethod::Fed => "fed",
        })
        .collect();
    parts.join(" + ")
}

/// Translate the (currently always-empty) post-action flash query.
///
/// In its current form this helper returns `None` unconditionally —
/// flashes on `/me/security/*` are surfaced via `?saved=1` style
/// query params (RFC 057), handled inside each tab. The helper
/// exists for API symmetry with the admin side; a future revision
/// may consolidate `?saved`/`?msg=…` translation here.
pub(super) fn flash_from_query(jar: &CookieJar) -> Option<sui_id_web::Flash> {
    let _ = jar;
    None
}
