//! Admin panel and login handlers.
//!
//! Split per screen domain (RFC 066, v0.47.1). The original
//! `admin.rs` was 1531 LOC and well over the spec's 500-LOC ceiling;
//! it now exists as this umbrella plus per-domain submodules.
//!
//! External callers (routes wired in `crate::router`) still reach
//! handlers through `crate::handlers::admin::handler_name` — each
//! submodule's `pub` items are flattened into the admin namespace
//! by the `pub use {submodule}::*;` re-exports below.
//!
//! Rust 2018+ module style is used throughout — this file is the
//! umbrella; submodules live in `admin/` as sibling .rs files. No
//! `mod.rs`.

use crate::state::AppState;
use axum::http::{header, HeaderValue};
use axum::response::Response;

pub mod forms;
mod auth;
mod dashboard;
mod users;
mod clients;
mod signing_keys;
mod audit;
mod webauthn;

pub use forms::{DisableForm, CsrfOnlyForm, ConfirmedForm, ConfirmedReasonForm};
pub use auth::*;
pub use dashboard::*;
pub use users::*;
pub use clients::*;
pub use signing_keys::*;
pub use audit::*;
pub use webauthn::*;

// ---------- umbrella-level helpers ----------

/// Attach a `Set-Cookie` header for the CSRF token to a response.
///
/// Used by every screen that renders a form (login, settings, user
/// detail, etc.). Lives at the umbrella level because moving it into
/// a submodule would force every sibling to `use super::with_csrf_cookie`
/// and lose nothing in clarity — this helper has no internal state.
pub(crate) fn with_csrf_cookie(mut resp: Response, app: &AppState, token: &str) -> Response {
    let cookie = crate::csrf::csrf_cookie(token.to_owned(), app.config.server.cookie_secure);
    if let Ok(v) = HeaderValue::from_str(&cookie.to_string()) {
        resp.headers_mut().append(header::SET_COOKIE, v);
    }
    resp
}

/// Render an `otpauth://` URI as an inline SVG QR code for the MFA
/// enrolment screen (TOTP setup, RFC 040).
///
/// Used both by `mfa_challenge_post` here in admin (called inline
/// where the secret is first generated) and by
/// `crate::handlers::me_security::mfa_enroll_start` through the
/// public wrapper below.
fn render_qr_svg(uri: &str) -> String {
    use qrcode::render::svg;
    use qrcode::QrCode;
    match QrCode::new(uri.as_bytes()) {
        Ok(code) => code
            .render::<svg::Color>()
            .min_dimensions(220, 220)
            .quiet_zone(true)
            .build(),
        Err(_) => "<p class=\"muted\">QR rendering failed; use the secret key below instead.</p>".to_string(),
    }
}

/// Public re-export of the QR-render helper for the self-service
/// MFA enrolment handler (RFC 055). The private `render_qr_svg`
/// stays module-internal; this wrapper exposes it without leaking
/// the rendering detail.
pub fn render_qr_svg_pub(uri: &str) -> String {
    render_qr_svg(uri)
}
