//! Step-up challenge endpoints.
//!
//! Mounted at `/me/security/step-up`. The user can satisfy the gate
//! either by entering a TOTP / recovery code, or by completing a
//! WebAuthn assertion against a registered passkey:
//!
//! - `GET  /me/security/step-up?return_to=<path>` — render the
//!   challenge page (TOTP form + WebAuthn button when applicable).
//! - `POST /me/security/step-up` — verify a TOTP / recovery code,
//!   touch `sessions.last_step_up_at`, redirect back to `return_to`.
//! - `POST /me/security/step-up/webauthn/start` — begin a WebAuthn
//!   ceremony; returns the challenge JSON for the browser.
//! - `POST /me/security/step-up/webauthn/finish` — verify the
//!   browser's assertion, touch `sessions.last_step_up_at`, redirect
//!   back to `return_to`.
//!
//! ## return_to validation
//!
//! `return_to` is a relative path on this origin. The handler
//! refuses anything else: not URL-shaped query strings (`http:` /
//! `https:` / `//` / `\\`), not protocol-relative URLs, not
//! arbitrary `?return_to=https://attacker.example/`. On any
//! suspicious input we silently fall back to `/me/security` —
//! never bouncing the user off-site after a successful auth is
//! the security guarantee.
//!
//! ## What we don't do
//!
//! - We do *not* offer "remember this device for 30 days". The
//!   freshness window is short (5 minutes) by design: it gates
//!   the *next* sensitive action, not a class of devices. If we
//!   wanted long-lived per-device trust we'd need a per-device
//!   token, an "untrust this device" UI, and a binding policy —
//!   all of which are independent features the simple step-up
//!   model can be extended into later if we have a need.

use crate::errors::HttpError;
use crate::handlers::{AppStateExt, SessionContext, enforce_csrf};
use crate::{csrf, handlers::admin::with_csrf_cookie};
use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Json, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use sui_id_core::errors::CoreError;
use sui_id_shared::ids::WebauthnPendingId;
use sui_id_web::{Flash, FlashKind};

/// Cookie name used to ferry the WebAuthn pending-ceremony id back
/// to the finish endpoint. HTTP-only and short-lived (5 minutes,
/// matching the pending row's TTL); cleared on success or failure.
const WEBAUTHN_STEP_UP_COOKIE: &str = "sui_id_step_up_webauthn_pending";

/// Route prefixes that may appear as a `?return_to=` value after a
/// successful step-up challenge (RFC 089).
///
/// Prefix matching: any path that starts with one of these values is
/// permitted. Every prefix is either under admin auth (`/admin/*`) or
/// self-service auth (`/me/security/*`), both of which are independently
/// session-gated, so prefix matching is safe.
const STEP_UP_RETURN_ALLOWLIST: &[&str] = &[
    "/admin/users/",
    "/admin/clients/",
    "/admin/signing-keys/",
    "/admin/settings/",
    "/me/security/",
];

/// Validate a `?return_to=` parameter for the step-up endpoint (RFC 089).
///
/// Acceptance criteria (all must pass):
/// 1. Non-empty, starts with a single `/` (not `//` or `/\`).
/// 2. Contains no backslash, CR, LF, or NUL.
/// 3. **Allowlist**: the path begins with one of the step-up-gated
///    route prefixes in `STEP_UP_RETURN_ALLOWLIST`.
///
/// Any path that fails any check collapses to `/me/security` so that
/// a crafted `?return_to=` cannot redirect the user to an arbitrary
/// route post-step-up.
fn sanitise_return_to(raw: &str) -> String {
    // ── Format checks (unchanged from pre-RFC-089) ──────────────────
    if raw.is_empty() {
        return "/me/security".to_owned();
    }
    if !raw.starts_with('/') {
        return "/me/security".to_owned();
    }
    if raw.starts_with("//") || raw.starts_with("/\\") {
        return "/me/security".to_owned();
    }
    for c in raw.chars() {
        if c == '\\' || c == '\n' || c == '\r' || c == '\0' {
            return "/me/security".to_owned();
        }
    }
    // ── RFC 089: allowlist check ─────────────────────────────────────
    // Reject any path not under a step-up-gated prefix, collapsing to
    // the safe default.  This prevents a crafted link from bouncing
    // the user to an unintended admin surface after a successful
    // step-up challenge.
    let in_allowlist = STEP_UP_RETURN_ALLOWLIST
        .iter()
        .any(|prefix| raw.starts_with(prefix));
    if !in_allowlist {
        return "/me/security".to_owned();
    }
    raw.to_owned()
}

#[derive(Debug, Deserialize)]
pub struct ReturnToQuery {
    #[serde(default)]
    pub return_to: String,
}

pub async fn get(
    state_ext: AppStateExt,
    ctx: SessionContext,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    Query(q): Query<ReturnToQuery>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    let return_to = sanitise_return_to(&q.return_to);
    let token = csrf::ensure_token(&jar);
    let has_passkey = sui_id_core::webauthn::has_credentials(&app.db, ctx.user_id)
        .await
        .map_err(HttpError::html)?;
    let html = sui_id_web::render_step_up(&return_to, token.clone(), has_passkey, None, lang);
    let resp = Html(html).into_response();
    Ok(with_csrf_cookie(resp, &app, &token))
}

#[derive(Debug, Deserialize)]
pub struct StepUpForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    pub code: String,
    #[serde(default)]
    pub return_to: String,
}

pub async fn post(
    state_ext: AppStateExt,
    ctx: SessionContext,
    crate::handlers::RequestLocale(lang): crate::handlers::RequestLocale,
    jar: CookieJar,
    axum::Form(form): axum::Form<StepUpForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    let return_to = sanitise_return_to(&form.return_to);
    let t = lang.strings();

    match sui_id_core::step_up::verify_totp_code(
        &app.db,
        &app.clock,
        ctx.user_id,
        ctx.session_id,
        &form.code,
    )
    .await
    {
        Ok(()) => {
            // The CSRF cookie was valid; rotate the in-process
            // token but don't burn a fresh one — the sensitive
            // action that follows will use its own CSRF check.
            Ok(Redirect::to(&return_to).into_response())
        }
        Err(CoreError::InvalidCredentials) => {
            let token = csrf::ensure_token(&jar);
            let has_passkey = sui_id_core::webauthn::has_credentials(&app.db, ctx.user_id)
                .await
                .map_err(HttpError::html)?;
            let flash = Flash {
                kind: FlashKind::Error,
                text: t.step_up_code_invalid.into(),
            };
            let html = sui_id_web::render_step_up(
                &return_to,
                token.clone(),
                has_passkey,
                Some(flash),
                lang,
            );
            let resp = (axum::http::StatusCode::BAD_REQUEST, Html(html)).into_response();
            Ok(with_csrf_cookie(resp, &app, &token))
        }
        Err(other) => Err(HttpError::html(other)),
    }
}

// ---------- WebAuthn step-up ----------

#[derive(Debug, Deserialize)]
pub struct WebauthnStartForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    #[serde(default)]
    pub return_to: String,
}

#[derive(Debug, Serialize)]
pub struct WebauthnStartResponse {
    /// JSON the browser hands to navigator.credentials.get(). We
    /// pass it as a string field so the client doesn't have to know
    /// our internal representation: it just JSON.parse()s this.
    pub challenge_json: String,
}

/// Begin a WebAuthn step-up ceremony. The handler stamps a short-
/// lived HTTP-only cookie with the pending row id; the browser
/// hands back the assertion to `webauthn_finish`, which reads the
/// pending id from the same cookie. We don't return the pending id
/// in the JSON because we don't want the browser to be able to
/// re-bind to a different ceremony's id.
pub async fn webauthn_start(
    state_ext: AppStateExt,
    ctx: SessionContext,
    jar: CookieJar,
    axum::Form(form): axum::Form<WebauthnStartForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    let _return_to = sanitise_return_to(&form.return_to); // validated for the
    // finish redirect, no
    // direct use here

    let started = sui_id_core::step_up::start_webauthn(
        &app.db,
        &app.clock,
        &app.config.server.issuer,
        ctx.user_id,
    )
    .await
    .map_err(HttpError::html)?;

    let pending_cookie = {
        let mut c = Cookie::new(WEBAUTHN_STEP_UP_COOKIE, started.pending_id.to_string());
        c.set_path("/");
        c.set_http_only(true);
        c.set_same_site(SameSite::Lax);
        c.set_secure(app.config.server.cookie_secure);
        c.set_max_age(cookie::time::Duration::minutes(5));
        c
    };
    let jar = jar.add(pending_cookie);
    let body = WebauthnStartResponse {
        challenge_json: started.challenge_json,
    };
    Ok((jar, Json(body)).into_response())
}

#[derive(Debug, Deserialize)]
pub struct WebauthnFinishForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    /// The PublicKeyCredential JSON from navigator.credentials.get(),
    /// stringified by the client.
    pub credential: String,
    #[serde(default)]
    pub return_to: String,
}

pub async fn webauthn_finish(
    state_ext: AppStateExt,
    ctx: SessionContext,
    jar: CookieJar,
    axum::Form(form): axum::Form<WebauthnFinishForm>,
) -> Result<Response, HttpError> {
    let State(app) = state_ext;
    enforce_csrf(&jar, Some(&form.csrf))?;
    let return_to = sanitise_return_to(&form.return_to);

    // Pull the pending id from the cookie. If it's missing or
    // malformed we treat the whole ceremony as failed and ask the
    // user to start over — same shape as webauthn-rs failures.
    let pending_id_str = jar
        .get(WEBAUTHN_STEP_UP_COOKIE)
        .map(|c| c.value().to_owned())
        .ok_or_else(|| HttpError::html(CoreError::InvalidCredentials))?;
    let pending_id: WebauthnPendingId = pending_id_str
        .parse()
        .map_err(|_| HttpError::html(CoreError::InvalidCredentials))?;

    let result = sui_id_core::step_up::finish_webauthn(
        &app.db,
        &app.clock,
        &app.config.server.issuer,
        ctx.user_id,
        ctx.session_id,
        pending_id,
        &form.credential,
    )
    .await;

    // Always clear the pending cookie — success and failure alike
    // burn the ceremony.
    let cleared = {
        let mut c = Cookie::new(WEBAUTHN_STEP_UP_COOKIE, "");
        c.set_path("/");
        c.set_http_only(true);
        c.set_same_site(SameSite::Lax);
        c.set_secure(app.config.server.cookie_secure);
        c.set_max_age(cookie::time::Duration::seconds(0));
        c
    };
    let jar = jar.add(cleared);

    match result {
        Ok(()) => Ok((jar, Redirect::to(&return_to)).into_response()),
        Err(_) => {
            // 400 with a JSON error so the client-side script can
            // surface it ("再認証に失敗しました。もう一度お試しください。")
            // without the page navigating away.
            let resp = (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "step_up_failed"})),
            )
                .into_response();
            Ok((jar, resp).into_response())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sanitise_return_to;

    // ── RFC 089: allowlist enforcement ────────────────────────────────────

    #[test]
    fn allowlisted_admin_users_path_accepted() {
        let result = sanitise_return_to("/admin/users/some-uuid/delete-confirm");
        assert_eq!(result, "/admin/users/some-uuid/delete-confirm");
    }

    #[test]
    fn allowlisted_admin_signing_keys_path_accepted() {
        let result = sanitise_return_to("/admin/signing-keys/some-id/delete-confirm");
        assert_eq!(result, "/admin/signing-keys/some-id/delete-confirm");
    }

    #[test]
    fn allowlisted_admin_settings_path_accepted() {
        let result = sanitise_return_to("/admin/settings/security");
        assert_eq!(result, "/admin/settings/security");
    }

    #[test]
    fn allowlisted_me_security_path_accepted() {
        let result = sanitise_return_to("/me/security/mfa");
        assert_eq!(result, "/me/security/mfa");
    }

    #[test]
    fn non_allowlisted_admin_dashboard_collapses_to_default() {
        let result = sanitise_return_to("/admin/dashboard");
        assert_eq!(
            result, "/me/security",
            "admin dashboard is not step-up gated"
        );
    }

    #[test]
    fn non_allowlisted_root_collapses_to_default() {
        let result = sanitise_return_to("/");
        assert_eq!(result, "/me/security");
    }

    #[test]
    fn non_allowlisted_admin_bare_collapses_to_default() {
        let result = sanitise_return_to("/admin");
        assert_eq!(result, "/me/security");
    }

    // ── Existing format-check tests (unchanged) ───────────────────────────

    #[test]
    fn empty_string_collapses() {
        assert_eq!(sanitise_return_to(""), "/me/security");
    }

    #[test]
    fn absolute_url_collapses() {
        assert_eq!(sanitise_return_to("https://evil.example/"), "/me/security");
    }

    #[test]
    fn protocol_relative_collapses() {
        assert_eq!(sanitise_return_to("//evil.example"), "/me/security");
    }

    #[test]
    fn backslash_prefix_collapses() {
        assert_eq!(sanitise_return_to("/\\evil"), "/me/security");
    }

    #[test]
    fn embedded_newline_collapses() {
        assert_eq!(
            sanitise_return_to("/admin/users/\nX-Header: injected"),
            "/me/security"
        );
    }

    #[test]
    fn embedded_nul_collapses() {
        assert_eq!(sanitise_return_to("/admin/users/\0etc"), "/me/security");
    }
}
