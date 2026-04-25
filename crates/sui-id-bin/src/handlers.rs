//! HTTP handlers.
//!
//! Each submodule covers one logical area. Helpers shared across them live
//! in this file directly.

use crate::errors::HttpError;
use crate::state::AppState;
use axum::extract::{FromRef, FromRequestParts, State};
use axum::http::request::Parts;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use std::str::FromStr;
use sui_id_core::errors::CoreError;
use sui_id_core::session;
use sui_id_shared::ids::{SessionId, UserId};
use sui_id_store::repos::users;

pub mod admin;
pub mod index;
pub mod oidc;
pub mod setup;

/// Cookie name for the admin / user session id.
pub const SESSION_COOKIE: &str = "sui_id_session";

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
        let user_id = session::resolve(&app.db, &app.clock, id).map_err(HttpError::html)?;
        Ok(CurrentUser(user_id))
    }
}

/// Like [`CurrentUser`] but additionally enforces administrator privilege.
pub struct CurrentAdmin(pub UserId);

impl<S> FromRequestParts<S> for CurrentAdmin
where
    S: Send + Sync,
    AppState: FromRef<S>,
{
    type Rejection = HttpError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let CurrentUser(uid) = CurrentUser::from_request_parts(parts, state).await?;
        let app: AppState = AppState::from_ref(state);
        let user = users::get(&app.db, uid).map_err(|_| HttpError::html(CoreError::Forbidden))?;
        if !user.is_admin || user.is_disabled || user.is_deleted {
            return Err(HttpError::html(CoreError::Forbidden));
        }
        Ok(CurrentAdmin(uid))
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
        let uid = session::resolve(&app.db, &app.clock, id).map_err(HttpError::api)?;
        let user = users::get(&app.db, uid).map_err(|_| HttpError::api(CoreError::Forbidden))?;
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
/// Prefers the real socket peer (`ConnectInfo<SocketAddr>` extension, set
/// by `into_make_service_with_connect_info` in `main`). Falls back to
/// `127.0.0.1` when the extension is missing — this happens when the
/// router is invoked via `tower::ServiceExt::oneshot` in tests, and would
/// otherwise force tests to set up a full TCP listener.
///
/// We deliberately do not consult `X-Forwarded-For` here. Doing so without
/// an explicit "trust proxy" configuration would let any caller bypass
/// per-IP rate limits by lying.
#[derive(Debug, Clone, Copy)]
pub struct ClientIp(pub std::net::IpAddr);

impl<S> axum::extract::FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        if let Some(axum::extract::ConnectInfo(addr)) = parts
            .extensions
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        {
            return Ok(Self(addr.ip()));
        }
        Ok(Self("127.0.0.1".parse().expect("static")))
    }
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
    let core_err = sui_id_core::CoreError::BadRequest(format!(
        "Too many requests. Try again in {} seconds.",
        decision.retry_after_secs
    ));
    let mut err = match representation {
        ErrorAs::Json => HttpError::api(core_err),
        ErrorAs::Html => HttpError::html(core_err),
    };
    err.set_retry_after_secs(decision.retry_after_secs);
    err.force_status(StatusCode::TOO_MANY_REQUESTS);
    Err(err)
}

#[derive(Debug, Clone, Copy)]
pub enum RateLimitKey {
    Login,
    Token,
    Setup,
}

impl RateLimitKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Token => "token",
            Self::Setup => "setup",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorAs {
    Json,
    Html,
}

use axum::http::StatusCode;
use sui_id_core::time::SharedClock;

// `cookie::time` is re-exported by the `cookie` crate which axum-extra
// depends on. We bring it in via the public path.
use cookie::time as cookie_time;
