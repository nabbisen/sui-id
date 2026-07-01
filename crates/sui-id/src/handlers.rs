//! HTTP handlers.
//!
//! Each submodule covers one logical area. Helpers shared across them live
//! in this file directly.

use crate::errors::HttpError;
use crate::state::AppState;
use axum::extract::{FromRef, FromRequestParts, State};
use axum::http::request::Parts;
use axum::response::IntoResponse;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use std::str::FromStr;
use sui_id_core::actor::{Actor, AdminActor, ReadOnlyAdminActor};
use sui_id_core::errors::CoreError;
use sui_id_core::session;
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::repos::users;

pub mod admin;
pub mod forgot_password;
pub mod index;
pub mod me_security;
pub mod oauth_token;
pub mod oidc;
pub mod settings;
pub mod setup;
pub mod step_up;

/// Cookie name for the in-flight WebAuthn ceremony id (used by both
/// registration and authentication challenges). HttpOnly because the
/// browser-side JS only needs the JSON challenge body, not the id.
pub const WEBAUTHN_PENDING_COOKIE: &str = "sui_id_webauthn_pending";

pub fn webauthn_pending_cookie<'a>(value: String, secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(WEBAUTHN_PENDING_COOKIE, value);
    c.set_path("/");
    c.set_http_only(true);
    c.set_same_site(SameSite::Lax);
    c.set_secure(secure);
    c.set_max_age(cookie_time::Duration::minutes(5));
    c
}

pub fn clear_webauthn_pending_cookie<'a>(secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(WEBAUTHN_PENDING_COOKIE, "");
    c.set_path("/");
    c.set_http_only(true);
    c.set_same_site(SameSite::Lax);
    c.set_secure(secure);
    c.set_max_age(cookie_time::Duration::seconds(0));
    c
}

/// Cookie name for the admin / user session id.
pub const SESSION_COOKIE: &str = "sui_id_session";

/// Cookie name for the short-lived "password OK, awaiting MFA" handle.
/// Set when login_post issues a `LoginOutcome::MfaRequired`; cleared when
/// the MFA challenge succeeds, fails terminally, or expires.
pub const PENDING_MFA_COOKIE: &str = "sui_id_pending_mfa";

/// Cookie name for the post-MFA redirect target. Carries the `next`
/// parameter the user supplied to the original login page across the
/// password → MFA challenge gap.
pub const PENDING_MFA_NEXT_COOKIE: &str = "sui_id_pending_mfa_next";

/// Build a session cookie. `secure` is configured by the operator.
pub fn session_cookie<'a>(value: String, secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(SESSION_COOKIE, value);
    c.set_path("/");
    c.set_http_only(true);
    c.set_same_site(SameSite::Lax);
    c.set_secure(secure);
    c
}

pub fn clear_session_cookie<'a>(secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(SESSION_COOKIE, "");
    c.set_path("/");
    c.set_http_only(true);
    c.set_same_site(SameSite::Lax);
    c.set_secure(secure);
    c.set_max_age(cookie_time::Duration::seconds(0));
    c
}

/// Build the pending-MFA cookie. HttpOnly because, like the session
/// cookie, the page never needs to read it from JavaScript.
pub fn pending_mfa_cookie<'a>(value: String, secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(PENDING_MFA_COOKIE, value);
    c.set_path("/");
    c.set_http_only(true);
    c.set_same_site(SameSite::Lax);
    c.set_secure(secure);
    // Same TTL as the underlying pending-mfa row.
    c.set_max_age(cookie_time::Duration::minutes(5));
    c
}

pub fn clear_pending_mfa_cookie<'a>(secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(PENDING_MFA_COOKIE, "");
    c.set_path("/");
    c.set_http_only(true);
    c.set_same_site(SameSite::Lax);
    c.set_secure(secure);
    c.set_max_age(cookie_time::Duration::seconds(0));
    c
}

pub fn pending_mfa_next_cookie<'a>(value: String, secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(PENDING_MFA_NEXT_COOKIE, value);
    c.set_path("/");
    c.set_http_only(true);
    c.set_same_site(SameSite::Lax);
    c.set_secure(secure);
    c.set_max_age(cookie_time::Duration::minutes(5));
    c
}

pub fn clear_pending_mfa_next_cookie<'a>(secure: bool) -> Cookie<'a> {
    let mut c = Cookie::new(PENDING_MFA_NEXT_COOKIE, "");
    c.set_path("/");
    c.set_http_only(true);
    c.set_same_site(SameSite::Lax);
    c.set_secure(secure);
    c.set_max_age(cookie_time::Duration::seconds(0));
    c
}

/// Extracted authenticated user. Pulls the session cookie, resolves it via
/// the core layer, and returns the user id.
pub struct CurrentUser(pub UserId);

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app: AppState = AppState::from_ref(state);
        let jar = CookieJar::from_headers(&parts.headers);
        let raw = jar.get(SESSION_COOKIE).ok_or_else(|| HttpError::html(CoreError::Unauthenticated))?;
        let id = SessionId::from_str(raw.value())
            .map_err(|_| HttpError::html(CoreError::Unauthenticated))?;
        let user_id = session::resolve(&app.db, &app.clock, id).await.map_err(HttpError::html)?;
        // Refresh the session's `last_used_at` so the v0.25.0
        // idle-timeout check has an accurate reference. Throttled
        // by the core layer (~one DB write per minute per
        // session); failures here do not affect auth — at worst
        // the row stays at its previous value and the next
        // request retries.
        let _ = session::touch_last_used(&app.db, &app.clock, id).await;
        Ok(CurrentUser(user_id))
    }
}

/// Like `CurrentUser` but also exposes the session id, so handlers
/// that need to operate on the session itself (revoke, step-up
/// touch) don't have to re-parse the cookie. Both fields are
/// validated through `session::resolve`, so by the time you have a
/// `SessionContext` the session is known to be live and to belong
/// to a real user.
pub struct SessionContext {
    pub user_id: UserId,
    pub session_id: SessionId,
}

impl<S> FromRequestParts<S> for SessionContext
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app: AppState = AppState::from_ref(state);
        let jar = CookieJar::from_headers(&parts.headers);
        let raw = jar
            .get(SESSION_COOKIE)
            .ok_or_else(|| HttpError::html(CoreError::Unauthenticated))?;
        let id = SessionId::from_str(raw.value())
            .map_err(|_| HttpError::html(CoreError::Unauthenticated))?;
        let user_id = session::resolve(&app.db, &app.clock, id).await.map_err(HttpError::html)?;
        let _ = session::touch_last_used(&app.db, &app.clock, id).await;
        Ok(SessionContext {
            user_id,
            session_id: id,
        })
    }
}

/// Like [`CurrentUser`] but additionally enforces administrator privilege
/// (write access). Returns 403 for auditors and plain users.
/// Used on all POST / DELETE / PUT admin routes.
/// RFC 081: produces an [`AdminActor`] alongside the raw `UserId`.
pub struct CurrentAdmin(pub UserId, pub AdminActor);

impl<S> FromRequestParts<S> for CurrentAdmin
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let SessionContext { user_id: uid, session_id } =
            SessionContext::from_request_parts(parts, state).await?;
        let app: AppState = AppState::from_ref(state);
        let user = users::get(&app.db, uid)
            .await
            .map_err(|_| HttpError::html(CoreError::Forbidden))?;
        // RFC 071: admins only for mutation surfaces.
        if !user.role.is_admin() || user.is_disabled || user.is_deleted {
            return Err(HttpError::html(CoreError::Forbidden));
        }
        // RFC 081: construct typed AdminActor.
        let actor = Actor::from_session(uid, user.role, session_id)
            .into_admin()
            .map_err(|_| HttpError::html(CoreError::Forbidden))?;
        Ok(CurrentAdmin(uid, actor))
    }
}

/// Admin OR auditor — passes for `role ∈ {admin, auditor}`.
/// Used on all GET admin routes so auditors can view without mutating.
/// Returns `(UserId, Role)` so handlers can pass role to render fns.
/// RFC 081: also produces a [`ReadOnlyAdminActor`] for domain read functions.
pub struct CurrentAdminOrAuditor(pub UserId, pub sui_id_store::models::Role, pub ReadOnlyAdminActor);

impl<S> FromRequestParts<S> for CurrentAdminOrAuditor
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let SessionContext { user_id: uid, session_id } =
            SessionContext::from_request_parts(parts, state).await?;
        let app: AppState = AppState::from_ref(state);
        let user = users::get(&app.db, uid)
            .await
            .map_err(|_| HttpError::html(CoreError::Forbidden))?;
        if !user.role.can_read_admin() || user.is_disabled || user.is_deleted {
            return Err(HttpError::html(CoreError::Forbidden));
        }
        // RFC 081: construct typed ReadOnlyAdminActor.
        let actor = Actor::from_session(uid, user.role, session_id)
            .into_read_admin()
            .map_err(|_| HttpError::html(CoreError::Forbidden))?;
        Ok(CurrentAdminOrAuditor(uid, user.role, actor))
    }
}

/// Same as [`CurrentAdmin`] but mapped to JSON-style errors. Used by the
/// JSON management API endpoints (currently none, but a useful seam if we
/// add one).
#[allow(dead_code)]
pub struct CurrentAdminJson(pub UserId);

impl<S> FromRequestParts<S> for CurrentAdminJson
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app: AppState = AppState::from_ref(state);
        let jar = CookieJar::from_headers(&parts.headers);
        let raw = jar.get(SESSION_COOKIE).ok_or_else(|| HttpError::api(CoreError::Unauthenticated))?;
        let id = SessionId::from_str(raw.value())
            .map_err(|_| HttpError::api(CoreError::Unauthenticated))?;
        let uid = session::resolve(&app.db, &app.clock, id).await.map_err(HttpError::api)?;
        let _ = session::touch_last_used(&app.db, &app.clock, id).await;
        let user = users::get(&app.db, uid).await.map_err(|_| HttpError::api(CoreError::Forbidden))?;
        if !user.is_admin || user.is_disabled || user.is_deleted {
            return Err(HttpError::api(CoreError::Forbidden));
        }
        Ok(CurrentAdminJson(uid))
    }
}

/// Convenience for handlers that need the app state shorthand.
pub type AppStateExt = State<AppState>;

/// Client IP address extractor.
///
/// Resolution order:
///
/// 1. Start with the socket peer (`ConnectInfo<SocketAddr>` extension).
/// 2. If `server.trusted_proxies` is non-empty *and* the peer is in that
///    set, walk `X-Forwarded-For` from rightmost to leftmost, dropping
///    every entry that is itself a trusted proxy. The first untrusted
///    address is the real client.
/// 3. Falls back to `127.0.0.1` only when the `ConnectInfo` extension is
///    missing entirely (e.g. tests using `oneshot`).
///
/// We deliberately do **not** read `X-Forwarded-For` when no proxies are
/// trusted: doing so would let any caller bypass per-IP rate limits by
/// supplying the header.
#[derive(Debug, Clone, Copy)]
pub struct ClientIp(pub std::net::IpAddr);

impl<S> axum::extract::FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
    AppState: axum::extract::FromRef<S>,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let app: AppState = AppState::from_ref(state);
        let peer = parts
            .extensions
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .map(|axum::extract::ConnectInfo(s)| s.ip());
        let peer = match peer {
            Some(ip) => ip,
            None => return Ok(Self("127.0.0.1".parse().expect("static"))),
        };
        if app.trusted_proxies.is_empty()
            || !crate::ipnet::any_contains(&app.trusted_proxies, &peer)
        {
            return Ok(Self(peer));
        }
        let xff = parts
            .headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|s| s.split(','))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        // Walk from rightmost to leftmost; the first entry that isn't a
        // trusted proxy is the client.
        for candidate in xff.iter().rev() {
            if let Ok(ip) = candidate.parse::<std::net::IpAddr>() {
                if !crate::ipnet::any_contains(&app.trusted_proxies, &ip) {
                    return Ok(Self(ip));
                }
            }
        }
        // The whole chain was trusted (unusual but legal); fall back to peer.
        Ok(Self(peer))
    }
}

/// Cookie name for the per-browser language preference. Set by
/// the profile handler when a signed-in user changes their
/// language; respected by the locale resolver as tier 2 of the
/// chain (between user.preferred_lang and Accept-Language).
pub const LANG_COOKIE: &str = "sui_id_lang";

/// Extractor that resolves the request's UI [`sui_id_i18n::Locale`]
/// using the four-tier chain in [`sui_id_core::i18n::resolve`]:
/// user preference → cookie → Accept-Language → server default.
///
/// The extractor reads the session cookie to determine the
/// signed-in user (if any); otherwise the user-tier is skipped
/// and the chain proceeds with cookie/header/default. It never
/// fails — on any DB error or missing input, it falls back to the
/// hard-coded default ([`sui_id_i18n::Locale::default`]).
#[derive(Debug, Clone, Copy)]
pub struct RequestLocale(pub sui_id_i18n::Locale);

impl<S> axum::extract::FromRequestParts<S> for RequestLocale
where
    S: Send + Sync,
    AppState: axum::extract::FromRef<S>,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let app: AppState = AppState::from_ref(state);

        // Pull the cookie jar manually so the extractor can be
        // composed alongside other cookie-using extractors without
        // moving the request body.
        let jar = axum_extra::extract::cookie::CookieJar::from_headers(&parts.headers);

        // Tier 1: signed-in user's preference. Best-effort —
        // we don't validate session expiry here (the locale path
        // is read-only and a stale session-id pointing at a real
        // user is fine). Auth gating happens in dedicated
        // extractors elsewhere.
        let user_id = {
            let sid_opt = jar
                .get(crate::handlers::SESSION_COOKIE)
                .and_then(|c| c.value().parse().ok());
            if let Some(sid) = sid_opt {
                sui_id_store::repos::sessions::get(&app.db, sid)
                    .await.ok()
                    .map(|row| row.user_id)
            } else {
                None
            }
        };

        // Tier 2: cookie.
        let cookie_lang = jar.get(LANG_COOKIE).map(|c| c.value().to_owned());

        // Tier 3: Accept-Language header.
        let accept_language = parts
            .headers
            .get(axum::http::header::ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        let inputs = sui_id_core::i18n::LocaleInputs {
            user_id,
            cookie: cookie_lang.as_deref(),
            accept_language: accept_language.as_deref(),
        };
        let locale = sui_id_core::i18n::resolve(&app.db, &inputs).await
            .unwrap_or_default();
        Ok(RequestLocale(locale))
    }
}

/// Enforce CSRF on a state-changing admin POST. Returns Err(403) when the
/// cookie is missing or does not match the form's `_csrf` field.
///
/// `form_token` is whatever the form-decoded body produced for the
/// `_csrf` field; the caller pulls this out of its own `Form<T>` struct
/// (each admin POST struct now carries a `csrf: String` field renamed
/// from the wire name `_csrf`).
pub fn enforce_csrf(
    jar: &axum_extra::extract::cookie::CookieJar,
    form_token: Option<&str>,
) -> Result<(), HttpError> {
    if crate::csrf::check_token(jar, form_token).is_some() {
        return Ok(());
    }
    tracing::warn!("CSRF token missing or mismatched on admin POST");
    let mut err = HttpError::html(sui_id_core::CoreError::Forbidden);
    err.force_status(axum::http::StatusCode::FORBIDDEN);
    Err(err)
}

/// Apply a rate-limit decision to a request. Returns `Err` (mapped to 429
/// with a `Retry-After` header) when the bucket is exhausted.
///
/// Pulls the client IP from the socket address; we deliberately do not trust
/// `X-Forwarded-For` here without an explicit configuration option — using a
/// header you do not control would let any caller bypass the limit by lying.
pub fn enforce_rate_limit(
    limiters: &crate::ratelimit::Limiters,
    clock: &SharedClock,
    key: RateLimitKey,
    ip: std::net::IpAddr,
    representation: ErrorAs,
) -> Result<(), HttpError> {
    let limiter = match key {
        RateLimitKey::Login => &limiters.login,
        RateLimitKey::Token => &limiters.token,
        RateLimitKey::Setup => &limiters.setup,
        RateLimitKey::ForgotPassword => &limiters.forgot_password,
    };
    let decision = limiter.check(key.as_str(), ip, clock.now());
    if decision.allowed {
        return Ok(());
    }
    tracing::warn!(
        ?key,
        %ip,
        retry_after = decision.retry_after_secs,
        "rate limit exceeded"
    );
    let core_err = sui_id_core::CoreError::Protocol {
        code: match representation {
            // For OAuth protocol endpoints, use the registered error code.
            ErrorAs::OAuth => sui_id_core::errors::ProtocolError::TemporarilyUnavailable,
            // For admin/UI endpoints, BadRequest is fine (humans see the message).
            _ => {
                let err = sui_id_core::CoreError::BadRequest(format!(
                    "Too many requests. Try again in {} seconds.",
                    decision.retry_after_secs
                ));
                let mut e = match representation {
                    ErrorAs::Json => HttpError::api(err),
                    ErrorAs::Html => HttpError::html(err),
                    ErrorAs::OAuth => unreachable!(),
                };
                e.set_retry_after_secs(decision.retry_after_secs);
                e.force_status(StatusCode::TOO_MANY_REQUESTS);
                return Err(e);
            }
        },
        description: format!(
            "Too many requests. Try again in {} seconds.",
            decision.retry_after_secs
        ),
    };
    let mut err = HttpError::oauth(core_err);
    err.set_retry_after_secs(decision.retry_after_secs);
    Err(err)
}

#[derive(Debug, Clone, Copy)]
pub enum RateLimitKey {
    Login,
    Token,
    Setup,
    ForgotPassword,
}

impl RateLimitKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Token => "token",
            Self::Setup => "setup",
            Self::ForgotPassword => "forgot_password",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorAs {
    Json,
    Html,
    /// RFC 6749 wire format: `{"error":"...","error_description":"..."}`.
    /// Use this for OAuth/OIDC protocol endpoints (token, introspect, revoke).
    /// Rate-limit errors become `{"error":"temporarily_unavailable",...}` with
    /// HTTP 429 and a `Retry-After` header.
    OAuth,
}

use axum::http::StatusCode;
use sui_id_core::time::SharedClock;

// `cookie::time` is re-exported by the `cookie` crate which axum-extra
// depends on. We bring it in via the public path.
use cookie::time as cookie_time;

// ---------- Step-up gate (v0.21.0) ----------
//
// `require_fresh_step_up` is the entry point handlers call right
// after extracting `SessionContext` and before doing the sensitive
// thing. It looks up the session's `last_step_up_at` and asks the
// core policy whether it's fresh. On Allow, it returns Ok and the
// caller proceeds. On Challenge, it returns an `Err(HttpError)`
// whose response is a 303 redirect to `/me/security/step-up?
// return_to=<current path>` — the user lands on the challenge form,
// completes a TOTP / passkey verify, and is bounced back to the
// original action.
//
// The freshness window default lives in `step_up::STEP_UP_FRESHNESS_SECS`
// (5 minutes). We expose it as a function rather than a const here so
// that an operator-controlled override can be wired in via Config
// later without touching every call site.

/// Gate the next action on a fresh step-up. Returns `Ok(())` if
/// the session has completed a step-up challenge within the
/// freshness window (or the user has no MFA enrolled, in which
/// case step-up is a no-op — see `step_up::policy_for_session`).
/// Returns `Err(Response)` carrying a 303 redirect to the
/// challenge page otherwise — the caller's idiom is:
///
/// ```ignore
/// if let Err(redirect) = require_fresh_step_up(&app, &ctx, "/me/security") {
///     return Ok(redirect);
/// }
/// // ...do the sensitive thing...
/// ```
///
/// We don't try to use `?` for this because the redirect isn't an
/// error in the application sense (the handler is doing exactly
/// what it should); it's just an alternative response.
pub async fn require_fresh_step_up(
    app: &AppState,
    ctx: &SessionContext,
    return_to: &str,
) -> Result<(), axum::response::Response> {
    let session_row = match sui_id_store::repos::sessions::get(&app.db, ctx.session_id).await {
        Ok(r) => r,
        Err(e) => {
            // Genuine DB error or the session vanished. Punt to
            // the auth-failed page rather than the step-up page —
            // it would be confusing to offer "verify yourself"
            // when we can't even read your session.
            return Err(HttpError::html(CoreError::from(e)).into_response());
        }
    };
    let decision = match sui_id_core::step_up::policy_for_session(
        &app.db,
        &app.clock,
        ctx.user_id,
        session_row.last_step_up_at,
        sui_id_core::step_up::STEP_UP_FRESHNESS_SECS,
    ).await {
        Ok(d) => d,
        Err(e) => return Err(HttpError::html(e).into_response()),
    };
    match decision {
        sui_id_core::step_up::StepUpDecision::Allow => Ok(()),
        sui_id_core::step_up::StepUpDecision::Challenge => {
            // We URL-encode the return path. `return_to` is a path
            // relative to our own origin; the step-up handler
            // validates that on the way out so a malicious
            // `?return_to=https://attacker.example/...` can't be
            // used to bounce a user off-site after a successful
            // challenge.
            let encoded: String =
                url::form_urlencoded::byte_serialize(return_to.as_bytes()).collect();
            let redirect = axum::response::Redirect::to(&format!(
                "/me/security/step-up?return_to={encoded}"
            ));
            Err(redirect.into_response())
        }
    }
}

/// Require that the request body contained `_confirmed=1` (RFC 030).
///
/// Prevents direct-POST bypass of the confirmation screen. The confirmation
/// page supplies `_confirmed=1`; raw form submissions without it are rejected
/// with a 400 Bad Request.
pub fn require_confirmed(confirmed: &str) -> Result<(), HttpError> {
    if confirmed == "1" {
        Ok(())
    } else {
        Err(HttpError::html(CoreError::BadRequest(
            "missing confirmation; use the confirmation screen".into(),
        )))
    }
}

/// Resolve the display locale for an admin panel response (RFC 029 § second pass).
///
/// Resolution order:
/// 1. The admin user's own `users.preferred_lang` (set in profile).
/// 2. `server_settings.default_lang` (operator-configured server default).
/// 3. `Locale::Ja` hardcoded fallback.
///
/// Errors in DB reads are silently ignored; the fallback guarantees a
/// usable locale even under degraded conditions.
pub async fn resolve_admin_locale(
    app: &crate::state::AppState,
    admin_id: sui_id_shared::ids::UserId,
) -> sui_id_i18n::Locale {
    // 1. Admin user's own preference
    if let Ok(user) = sui_id_store::repos::users::get(&app.db, admin_id).await {
        if let Some(ref tag) = user.preferred_lang {
            if let Some(loc) = sui_id_i18n::Locale::parse(tag) {
                return loc;
            }
        }
    }

    // 2. Server-configured default language
    if let Ok(settings) = sui_id_store::repos::server_settings::get(&app.db).await {
        if let Some(loc) = sui_id_i18n::Locale::parse(&settings.default_lang) {
            return loc;
        }
    }

    // 3. Hardcoded fallback
    sui_id_i18n::Locale::Ja
}

/// Effective minimum password length for the current run mode.
///
/// Delegates to `AppState::security_level().password_min_len()` so
/// handler code stays concise and consistent with other level-driven
/// thresholds.
#[inline]
pub fn password_min_len(app: &crate::state::AppState) -> usize {
    app.security_level().password_min_len()
}
