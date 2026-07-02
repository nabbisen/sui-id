//! OIDC and OAuth 2.0 protocol endpoints.

use crate::errors::HttpError;
use crate::handlers::AppStateExt;
use axum::Form;
use axum::Json;
use axum::extract::Query;
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Redirect, Response};
use base64ct::{Base64, Encoding};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use sui_id_core::authorize::{
    self, AuthorizeParams, CodeExchangeRequest, IssuanceContext, RefreshExchangeRequest,
};
use sui_id_core::discovery::Discovery;
use sui_id_core::errors::{CoreError, ProtocolError};
use sui_id_core::jwks;
use sui_id_shared::RawRefreshToken;
use sui_id_shared::ids::ClientId;
use sui_id_shared::ids::UserId;
use sui_id_store::models::ConsentPolicy;
use sui_id_store::repos::users;
use sui_id_web::{pages::ConsentData, render_consent};

// ---------- discovery & JWKS ----------

pub async fn discovery(state_ext: AppStateExt) -> impl IntoResponse {
    let axum::extract::State(app) = state_ext;
    Json(Discovery::build(app.issuer()))
}

pub async fn jwks(state_ext: AppStateExt) -> Result<Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let body = jwks::build(&app.db)
        .await
        .map_err(|e| HttpError::api(CoreError::from(e)))?;
    let mut resp = Json(body).into_response();
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        "public, max-age=300"
            .parse()
            .unwrap_or_else(|_| "no-store".parse().expect("static")),
    );
    Ok(resp)
}

// ---------- /authorize ----------

#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub nonce: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: String,
}

/// `GET /oauth2/authorize`. Three-phase flow:
///
/// **Phase 1 — client validation (before login).**
/// Validates `client_id` and `redirect_uri` against the database.
/// On failure: renders an HTML error page and stops — we must never
/// redirect to an untrusted `redirect_uri` (RFC 6749 §4.1.2.1).
///
/// **Phase 2 — authentication.**
/// If the user has no session, redirects to `/admin/login?next=...`.
/// On return, execution continues at phase 3.
///
/// **Phase 3 — request validation and code issuance.**
/// Validates `response_type`, PKCE, and scope. On failure: redirects
/// to `redirect_uri?error=...&state=...` (RFC 6749 §4.1.2.1). The
/// `redirect_uri` is already confirmed valid at this point.
pub async fn authorize(
    state_ext: AppStateExt,
    Query(q): Query<AuthorizeQuery>,
    headers: HeaderMap,
) -> Result<Response, HttpError> {
    let axum::extract::State(app) = state_ext;

    // ── Phase 1: validate client + redirect_uri BEFORE any login redirect.
    // Showing the error here avoids asking the user to authenticate only to
    // land on an error page they cannot resolve.
    let client_id = ClientId::from_str(&q.client_id).map_err(|_| {
        HttpError::html(CoreError::Protocol {
            code: ProtocolError::InvalidClient,
            description: "client_id is not a valid identifier".into(),
        })
    })?;
    authorize::validate_client_and_redirect_uri(&app.db, client_id, &q.redirect_uri)
        .await
        .map_err(|e| {
            tracing::warn!(
                client_id = %q.client_id,
                redirect_uri = %q.redirect_uri,
                error = %e,
                "authorize: client or redirect_uri rejected (HTML error, no redirect)"
            );
            HttpError::html(e)
        })?;

    // ── Phase 2: ensure the user is authenticated.
    let (session_user, session_auth_methods) = match resolve_session(&app, &headers).await {
        Some(state) => state,
        None => {
            let qs = build_query_string(&q);
            let next = format!("/oauth2/authorize?{qs}");
            let encoded = utf8_percent_encode(&next, NON_ALPHANUMERIC).to_string();
            return Ok(Redirect::to(&format!("/admin/login?next={encoded}")).into_response());
        }
    };

    // ── Phase 3: validate the remaining request parameters and issue the code.
    // redirect_uri is confirmed valid from Phase 1; RFC 6749 §4.1.2.1 allows
    // (and expects) error responses to be delivered via redirect from this point.
    let scope = q.scope.clone().unwrap_or_else(|| "openid".into());
    let params = AuthorizeParams {
        client_id,
        redirect_uri: q.redirect_uri.clone(),
        response_type: q.response_type.clone(),
        scope,
        state: q.state.clone(),
        nonce: q.nonce.clone(),
        code_challenge: q.code_challenge.clone(),
        code_challenge_method: q.code_challenge_method.clone(),
    };

    let accepted = match authorize::begin_authorization(&app.db, params).await {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(
                client_id = %q.client_id,
                error = %e,
                "authorize: request parameters rejected (redirecting with error)"
            );
            // Redirect the error back to the RP — the redirect_uri is trusted.
            return Ok(protocol_error_redirect(
                &q.redirect_uri,
                q.state.as_deref(),
                e,
            ));
        }
    };

    // Consent gate (RFC 038) — look up client to check consent_policy.
    let client_for_consent = sui_id_store::repos::clients::get(&app.db, accepted.params.client_id)
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;
    let scope = accepted.params.scope.clone();
    let needs_consent = match client_for_consent.consent_policy {
        ConsentPolicy::None => false,
        ConsentPolicy::Always => true,
        ConsentPolicy::FirstTime => {
            let stored = sui_id_store::repos::user_consent::get(
                &app.db,
                session_user,
                client_for_consent.id,
            )
            .await
            .unwrap_or(None);
            match stored {
                Some(row)
                    if sui_id_store::repos::user_consent::covers(&row.granted_scopes, &scope) =>
                {
                    false
                }
                _ => true,
            }
        }
    };

    if needs_consent {
        let lang = sui_id_store::repos::server_settings::get(&app.db)
            .await
            .ok()
            .and_then(|s| sui_id_i18n::Locale::parse(&s.default_lang))
            .unwrap_or_default();

        // Serialise the accepted params into a short-lived cookie so the
        // consent POST can complete the flow without replaying /authorize.
        let cs = serde_json::json!({
            "user_id": session_user.to_string(),
            "client_id": accepted.params.client_id.to_string(),
            "redirect_uri": accepted.params.redirect_uri,
            "scope": scope,
            "state": accepted.params.state,
            "nonce": accepted.params.nonce,
            "code_challenge": accepted.params.code_challenge,
            "code_challenge_method": accepted.params.code_challenge_method,
            "auth_methods": serde_json::to_value(&session_auth_methods).unwrap_or_default(),
        });
        let cs_json =
            serde_json::to_string(&cs).map_err(|_| HttpError::html(CoreError::Internal))?;

        let scopes: Vec<String> = scope.split_whitespace().map(|s| s.to_string()).collect();
        let csrf_tok = crate::csrf::new_token();
        let consent_html = render_consent(
            ConsentData {
                client_name: client_for_consent.name.clone(),
                requested_scopes: scopes,
                csrf_token: csrf_tok.clone(),
                // RFC 008: application identity from ClientRow.
                logo_uri: client_for_consent.logo_uri.clone(),
                homepage_uri: client_for_consent.homepage_uri.clone(),
            },
            lang,
        );

        let cookie_val =
            format!("sui_id_consent={cs_json}; HttpOnly; SameSite=Lax; Max-Age=300; Path=/");
        let csrf_cookie_val = format!("sui_id_csrf={csrf_tok}; SameSite=Lax; Max-Age=300; Path=/");
        let mut resp = axum::response::Response::new(axum::body::Body::from(consent_html));
        *resp.status_mut() = axum::http::StatusCode::OK;
        resp.headers_mut().append(
            axum::http::header::SET_COOKIE,
            axum::http::HeaderValue::from_str(&cookie_val)
                .map_err(|_| HttpError::html(CoreError::Internal))?,
        );
        resp.headers_mut().append(
            axum::http::header::SET_COOKIE,
            axum::http::HeaderValue::from_str(&csrf_cookie_val)
                .map_err(|_| HttpError::html(CoreError::Internal))?,
        );
        resp.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("text/html; charset=utf-8"),
        );
        return Ok(resp);
    }

    let redirect = authorize::complete_authorization(
        &app.db,
        &app.clock,
        session_user,
        &session_auth_methods,
        accepted,
    )
    .await
    .map_err(HttpError::html)?;

    let mut url = redirect.redirect_uri;
    let sep = if url.contains('?') { '&' } else { '?' };
    url.push(sep);
    url.push_str("code=");
    url.push_str(&utf8_percent_encode(&redirect.code, NON_ALPHANUMERIC).to_string());
    if let Some(s) = redirect.state {
        url.push_str("&state=");
        url.push_str(&utf8_percent_encode(&s, NON_ALPHANUMERIC).to_string());
    }
    Ok(Redirect::to(&url).into_response())
}

async fn resolve_session(
    app: &crate::AppState,
    headers: &HeaderMap,
) -> Option<(sui_id_shared::ids::UserId, Vec<sui_id_shared::AuthMethod>)> {
    use axum_extra::extract::cookie::CookieJar;
    use sui_id_shared::ids::SessionId;
    let jar = CookieJar::from_headers(headers);
    let raw = jar
        .get(crate::handlers::SESSION_COOKIE)?
        .value()
        .to_string();
    let id = SessionId::from_str(&raw).ok()?;
    // Resolve hits the database both for validity (expiry / revocation)
    // and to fetch the recorded auth_methods. Cheaper to do it once
    // here than to call resolve and then re-fetch the row.
    let session = sui_id_store::repos::sessions::get(&app.db, id).await.ok()?;
    if session.revoked_at.is_some() || session.expires_at < app.clock.now() {
        return None;
    }
    Some((session.user_id, session.auth_methods))
}

/// Build an RFC 6749 §4.1.2.1 error redirect response.
///
/// Used when `client_id` and `redirect_uri` have been confirmed valid
/// (Phase 1 succeeded) but a request-level parameter failed (Phase 3).
/// The `state` parameter, if present, must be echoed back verbatim.
fn protocol_error_redirect(
    redirect_uri: &str,
    state: Option<&str>,
    err: sui_id_core::errors::CoreError,
) -> Response {
    let (code_str, description) = match err {
        sui_id_core::errors::CoreError::Protocol { code, description } => {
            (code.as_str(), description)
        }
        other => ("server_error", other.to_string()),
    };
    let mut url = format!("{redirect_uri}?error={code_str}");
    url.push_str("&error_description=");
    url.push_str(&utf8_percent_encode(&description, NON_ALPHANUMERIC).to_string());
    if let Some(s) = state {
        url.push_str("&state=");
        url.push_str(&utf8_percent_encode(s, NON_ALPHANUMERIC).to_string());
    }
    Redirect::to(&url).into_response()
}

fn build_query_string(q: &AuthorizeQuery) -> String {
    let mut parts: Vec<(&str, String)> = vec![
        ("client_id", q.client_id.clone()),
        ("redirect_uri", q.redirect_uri.clone()),
        ("response_type", q.response_type.clone()),
        ("code_challenge", q.code_challenge.clone()),
        ("code_challenge_method", q.code_challenge_method.clone()),
    ];
    if let Some(s) = &q.scope {
        parts.push(("scope", s.clone()));
    }
    if let Some(s) = &q.state {
        parts.push(("state", s.clone()));
    }
    if let Some(n) = &q.nonce {
        parts.push(("nonce", n.clone()));
    }
    parts
        .into_iter()
        .map(|(k, v)| format!("{k}={}", utf8_percent_encode(&v, NON_ALPHANUMERIC)))
        .collect::<Vec<_>>()
        .join("&")
}

// ---------- /token ----------

#[derive(Debug, Deserialize)]
pub struct TokenForm {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub refresh_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    pub scope: Option<String>,
}

pub async fn token(
    state_ext: AppStateExt,
    crate::handlers::ClientIp(ip): crate::handlers::ClientIp,
    headers: HeaderMap,
    Form(form): Form<TokenForm>,
) -> Result<Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    crate::handlers::enforce_rate_limit(
        &app.limiters,
        &app.clock,
        crate::handlers::RateLimitKey::Token,
        ip,
        crate::handlers::ErrorAs::OAuth,
    )?;

    let basic = parse_basic_auth(&headers);
    let (header_client, header_secret) = match basic {
        Some((c, s)) => (Some(c), Some(s)),
        None => (None, None),
    };
    let client_id_raw = header_client
        .or_else(|| form.client_id.clone())
        .ok_or_else(|| {
            HttpError::oauth(CoreError::Protocol {
                code: ProtocolError::InvalidClient,
                description: "client_id is required".into(),
            })
        })?;
    let client_id = ClientId::from_str(&client_id_raw).map_err(|_| {
        HttpError::oauth(CoreError::Protocol {
            code: ProtocolError::InvalidClient,
            description: "client_id is not a valid identifier".into(),
        })
    })?;
    let client_secret = header_secret.or_else(|| form.client_secret.clone());

    let lifetimes = app.token_lifetimes();
    let issuer = app.issuer().to_owned();
    let ctx = IssuanceContext {
        issuer: &issuer,
        lifetimes,
    };

    let set = match form.grant_type.as_str() {
        "authorization_code" => {
            let code = form.code.ok_or_else(|| {
                HttpError::oauth(CoreError::Protocol {
                    code: ProtocolError::InvalidRequest,
                    description: "code is required".into(),
                })
            })?;
            let redirect_uri = form.redirect_uri.ok_or_else(|| {
                HttpError::oauth(CoreError::Protocol {
                    code: ProtocolError::InvalidRequest,
                    description: "redirect_uri is required".into(),
                })
            })?;
            let code_verifier = form.code_verifier.ok_or_else(|| {
                HttpError::oauth(CoreError::Protocol {
                    code: ProtocolError::InvalidRequest,
                    description: "code_verifier is required (PKCE)".into(),
                })
            })?;
            authorize::exchange_code(
                &app.db,
                &app.clock,
                ctx,
                CodeExchangeRequest {
                    code,
                    redirect_uri,
                    client_id,
                    client_secret,
                    code_verifier,
                },
            )
            .await
            .map_err(HttpError::oauth)?
        }
        "refresh_token" => {
            let refresh_token = form.refresh_token.ok_or_else(|| {
                HttpError::oauth(CoreError::Protocol {
                    code: ProtocolError::InvalidRequest,
                    description: "refresh_token is required".into(),
                })
            })?;
            authorize::exchange_refresh(
                &app.db,
                &app.clock,
                ctx,
                RefreshExchangeRequest {
                    refresh_token: RawRefreshToken::from_untrusted(refresh_token),
                    client_id,
                    client_secret,
                },
            )
            .await
            .map_err(HttpError::oauth)?
        }
        other => {
            return Err(HttpError::oauth(CoreError::Protocol {
                code: ProtocolError::UnsupportedGrantType,
                description: format!("unsupported grant_type: {other}"),
            }));
        }
    };

    let resp = TokenResponse {
        access_token: set.access_token,
        token_type: "Bearer",
        expires_in: set.access_expires_in,
        // expose() is the single, intentional plaintext egress point for
        // the refresh token — immediately after it is inserted into the DB
        // and before it is handed to the client. The RawRefreshToken is
        // dropped at end of this scope.
        refresh_token: set.refresh_token.expose().to_owned(),
        id_token: set.id_token,
        scope: None,
    };

    // RFC 006: count token issuances.
    if let Some(m) = app.metric() {
        m.token_issued(sui_id_store::metrics::token_kind::ACCESS);
        m.token_issued(sui_id_store::metrics::token_kind::REFRESH);
        if resp.id_token.is_some() {
            m.token_issued(sui_id_store::metrics::token_kind::ID);
        }
    }

    // RFC 072: update last_used_at on the consent grant so /me/apps can
    // show "Last used: …". Best-effort — id_token is only present when
    // the user_id is known (authorization_code or refresh_token exchange
    // for a public/confidential client with openid scope). For exchanges
    // without an id_token (client_credentials, device_code — none shipped)
    // this is a no-op.
    if let Some(user_id) = set.user_id {
        let _ = sui_id_store::repos::user_consent::touch_last_used(
            &app.db,
            user_id,
            client_id,
            app.clock.now(),
        )
        .await;
    }
    let mut out = Json(resp).into_response();
    out.headers_mut().insert(
        header::CACHE_CONTROL,
        "no-store".parse().expect("static header value"),
    );
    out.headers_mut().insert(
        header::PRAGMA,
        "no-cache".parse().expect("static header value"),
    );
    Ok(out)
}

pub(crate) fn parse_basic_auth(headers: &HeaderMap) -> Option<(String, String)> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let body = raw.strip_prefix("Basic ")?;
    let mut buf = vec![0u8; body.len()];
    let n = Base64::decode(body, &mut buf).ok()?.len();
    buf.truncate(n);
    let s = String::from_utf8(buf).ok()?;
    let (id, secret) = s.split_once(':')?;
    Some((
        percent_decode(id).unwrap_or_else(|| id.to_owned()),
        percent_decode(secret).unwrap_or_else(|| secret.to_owned()),
    ))
}

fn percent_decode(s: &str) -> Option<String> {
    percent_encoding::percent_decode_str(s)
        .decode_utf8()
        .ok()
        .map(|c| c.into_owned())
}

// ---------- /userinfo ----------

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub sub: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Original-case email address. Returned only when the access token's
    /// granted scope includes `email` and the user has an email on record.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Whether the email has been confirmed (`email_verified_at IS NOT NULL`).
    /// Omitted entirely when `email` is not returned (OIDC convention).
    /// Always `false` until an email-verification flow is implemented.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
}

/// Standard OIDC userinfo endpoint. Authenticates via the Bearer access
/// token: we verify the JWT signature against our own JWKS, check expiry,
/// and look up the subject.
pub async fn userinfo(state_ext: AppStateExt, headers: HeaderMap) -> Result<Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let raw = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| HttpError::api(CoreError::Unauthenticated))?;

    let claims = sui_id_core::tokens::verify_access_token_cached(&app.caches, &app.clock, raw)
        .await
        .map_err(HttpError::api)?;
    // RFC 7009: a revoked access token must stop being honoured at
    // protected endpoints. We consult the deny-list before serving.
    if sui_id_store::repos::revoked_access_tokens::is_revoked(&app.db, &claims.jti)
        .await
        .map_err(|e| HttpError::api(CoreError::from(e)))?
    {
        return Err(HttpError::api(CoreError::Unauthenticated));
    }
    let uid: sui_id_shared::ids::UserId = claims
        .sub
        .parse()
        .map_err(|_| HttpError::api(CoreError::Internal))?;
    let row = users::get(&app.db, uid)
        .await
        .map_err(|_| HttpError::api(CoreError::Unauthenticated))?;
    if row.is_disabled || row.is_deleted {
        return Err(HttpError::api(CoreError::Unauthenticated));
    }
    // OIDC Core §5.3.2 SHOULD: userinfo is per-user data and must
    // not be cached by intermediaries. Without `no-store` a CDN or
    // shared proxy could serve one user's claims to another.
    // Populate email claims when (a) the access token's scope includes
    // "email" and (b) the user has an email address on record.
    let scope_includes_email = claims.scope.split_whitespace().any(|s| s == "email");
    let (email_claim, email_verified_claim) = if scope_includes_email {
        match row.email.as_deref() {
            Some(addr) => {
                let verified = row.email_verified_at.is_some();
                (Some(addr.to_owned()), Some(verified))
            }
            None => (None, None),
        }
    } else {
        (None, None)
    };

    let mut resp = Json(UserInfo {
        sub: row.id.to_string(),
        preferred_username: Some(row.username),
        name: row.display_name,
        email: email_claim,
        email_verified: email_verified_claim,
    })
    .into_response();
    resp.headers_mut().insert(
        header::CACHE_CONTROL,
        "no-store".parse().expect("static header value"),
    );
    Ok(resp)
}

// ---------- /logout (RP-initiated).await ----------

/// Query parameters for the RP-initiated logout endpoint, per
/// OpenID Connect RP-Initiated Logout 1.0.
#[derive(Debug, Deserialize)]
pub struct LogoutQuery {
    /// Optional ID Token previously issued to this RP. Used as a hint for
    /// which user to log out and verified against our JWKS.
    pub id_token_hint: Option<String>,
    /// Where to send the user after logout. Must match a `redirect_uris`
    /// entry on the client referenced by `id_token_hint`. Without a hint
    /// we cannot validate against a client and must reject the parameter.
    pub post_logout_redirect_uri: Option<String>,
    /// Opaque value echoed back to the RP for CSRF protection.
    pub state: Option<String>,
    /// Optional `client_id`. Some RPs send it explicitly; we accept it as
    /// a fallback when there is no `id_token_hint`.
    pub client_id: Option<String>,
}

/// `GET /oauth2/logout`. End the user's session and, if a valid
/// `post_logout_redirect_uri` was supplied, redirect to it.
pub async fn logout(
    state_ext: AppStateExt,
    Query(q): Query<LogoutQuery>,
    headers: HeaderMap,
) -> Result<Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let mut user_id: Option<sui_id_shared::ids::UserId> = None;
    let mut hinted_client: Option<sui_id_shared::ids::ClientId> = None;

    // Prefer the id_token_hint when present.
    if let Some(token) = &q.id_token_hint {
        // Verify the signature; accept expired tokens so the user can still
        // log out after their session has gone stale.
        if let Ok(claims) =
            sui_id_core::tokens::verify_id_token_cached(&app.caches, &app.clock, token, true).await
        {
            user_id = claims.sub.parse().ok();
            hinted_client = claims.aud.parse().ok();
        } else {
            tracing::warn!("logout: id_token_hint failed signature verification");
        }
    }

    // Fall back to the session cookie for the user identification.
    if user_id.is_none() {
        if let Some(cookie) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
            for part in cookie.split(';') {
                let part = part.trim();
                if let Some(value) = part.strip_prefix("sui_id_session=") {
                    if let Ok(sid) = value.parse() {
                        if let Ok(uid) =
                            sui_id_core::session::resolve(&app.db, &app.clock, sid).await
                        {
                            user_id = Some(uid);
                        }
                    }
                }
            }
        }
    }

    if let Some(uid) = user_id {
        sui_id_core::session::logout_user(&app.db, &app.clock, uid)
            .await
            .map_err(HttpError::html)?;
    }

    // Validate the post_logout_redirect_uri against the hinted client.
    //
    // Resolution order:
    //   1. Match against the client's own `post_logout_redirect_uris`
    //      (added in v0.6.0 via migration 0002). This is the standards-
    //      blessed list: an RP that registered logout URIs explicitly
    //      should only receive logout redirects to those URIs.
    //   2. If the client has *no* logout URIs registered, fall back to
    //      its `redirect_uris` for backwards compatibility with clients
    //      that pre-date this feature. We log a deprecation note when
    //      the fallback is taken.
    let redirect_target = match (
        q.post_logout_redirect_uri.as_deref(),
        hinted_client.or_else(|| q.client_id.as_deref().and_then(|s| s.parse().ok())),
    ) {
        (Some(uri), Some(cid)) => match sui_id_store::repos::clients::get(&app.db, cid).await {
            Ok(client) if !client.is_disabled && !client.is_deleted => {
                if !client.post_logout_redirect_uris.is_empty() {
                    if client.post_logout_redirect_uris.iter().any(|u| u == uri) {
                        Some(uri.to_owned())
                    } else {
                        None
                    }
                } else if client.redirect_uris.iter().any(|u| u == uri) {
                    tracing::warn!(
                        client_id = %cid,
                        "logout: falling back to redirect_uris for post_logout_redirect_uri \
                         match (deprecated; register post_logout_redirect_uris on the client)"
                    );
                    Some(uri.to_owned())
                } else {
                    None
                }
            }
            _ => None,
        },
        _ => None,
    };

    // Always clear the session cookie before redirecting / responding.
    let clear_cookie = format!(
        "sui_id_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
        if app.config.server.cookie_secure {
            "; Secure"
        } else {
            ""
        }
    );

    if let Some(mut url) = redirect_target {
        if let Some(state) = q.state.as_deref() {
            let sep = if url.contains('?') { '&' } else { '?' };
            url.push(sep);
            url.push_str("state=");
            url.push_str(&utf8_percent_encode(state, NON_ALPHANUMERIC).to_string());
        }
        let mut resp = Redirect::to(&url).into_response();
        if let Ok(v) = axum::http::HeaderValue::from_str(&clear_cookie) {
            resp.headers_mut().insert(header::SET_COOKIE, v);
        }
        return Ok(resp);
    }

    // No (or invalid) post_logout_redirect_uri: render a small confirmation page.
    let body = "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>Signed out</title></head>\
                <body><h1>Signed out</h1><p>You have been signed out of sui-id.</p>\
                <p><a href=\"/admin/login\">Sign in again</a></p></body></html>";
    let mut resp = (axum::http::StatusCode::OK, axum::response::Html(body)).into_response();
    if let Ok(v) = axum::http::HeaderValue::from_str(&clear_cookie) {
        resp.headers_mut().insert(header::SET_COOKIE, v);
    }
    Ok(resp)
}

// ---------- /oauth2/consent (RFC 038) ----------

#[derive(Debug, serde::Deserialize)]
pub struct ConsentForm {
    #[serde(rename = "_csrf", default)]
    pub csrf: String,
    /// "approve" or "deny"
    pub decision: String,
}

pub async fn consent_get(
    state_ext: AppStateExt,
    jar: axum_extra::extract::cookie::CookieJar,
) -> Result<Response, HttpError> {
    // The GET just re-renders the consent page if the cookie is present.
    // Usually users arrive here via POST-redirect, not direct GET.
    let _ = state_ext;
    let _ = jar;
    Ok(axum::response::Redirect::to("/").into_response())
}

pub async fn consent_post(
    state_ext: AppStateExt,
    jar: axum_extra::extract::cookie::CookieJar,
    axum::Form(form): axum::Form<ConsentForm>,
) -> Result<Response, HttpError> {
    let axum::extract::State(app) = state_ext;

    // Read the consent session cookie
    let cs_json = jar
        .get("sui_id_consent")
        .map(|c| c.value().to_string())
        .ok_or_else(|| {
            HttpError::html(CoreError::BadRequest(
                "consent session expired or missing; restart the login flow".into(),
            ))
        })?;

    // Validate CSRF
    crate::handlers::enforce_csrf(&jar, Some(&form.csrf))?;

    let cs: serde_json::Value = serde_json::from_str(&cs_json)
        .map_err(|_| HttpError::html(CoreError::BadRequest("invalid consent session".into())))?;

    let get_str = |key: &str| cs[key].as_str().unwrap_or("").to_string();

    // Handle deny
    if form.decision != "approve" {
        let redirect_uri = get_str("redirect_uri");
        let state = cs["state"].as_str().map(|s| s.to_string());
        let sep = if redirect_uri.contains('?') { '&' } else { '?' };
        let mut url = format!("{redirect_uri}{sep}error=access_denied");
        if let Some(s) = state {
            url.push_str("&state=");
            url.push_str(&s);
        }
        return Ok(axum::response::Redirect::to(&url).into_response());
    }

    // Parse the session params and re-run complete_authorization
    let user_id = get_str("user_id").parse::<UserId>().map_err(|_| {
        HttpError::html(CoreError::BadRequest(
            "invalid user_id in consent session".into(),
        ))
    })?;
    let client_id = get_str("client_id").parse::<ClientId>().map_err(|_| {
        HttpError::html(CoreError::BadRequest(
            "invalid client_id in consent session".into(),
        ))
    })?;
    let scope = get_str("scope");

    // Store the consent grant
    sui_id_store::repos::user_consent::upsert(&app.db, user_id, client_id, scope.clone())
        .await
        .map_err(|e| HttpError::html(CoreError::from(e)))?;

    // Reconstruct auth_methods from cookie
    let auth_methods: Vec<sui_id_shared::AuthMethod> =
        serde_json::from_value(cs["auth_methods"].clone()).unwrap_or_default();

    let params = sui_id_core::authorize::AuthorizeParams {
        client_id,
        redirect_uri: get_str("redirect_uri"),
        response_type: "code".into(),
        scope: scope.clone(),
        state: cs["state"].as_str().map(|s| s.to_string()),
        nonce: cs["nonce"].as_str().map(|s| s.to_string()),
        code_challenge: get_str("code_challenge"),
        code_challenge_method: get_str("code_challenge_method"),
    };

    let accepted = match sui_id_core::authorize::begin_authorization(&app.db, params).await {
        Ok(a) => a,
        Err(e) => {
            // redirect_uri is from the consent cookie (already validated at consent
            // display time), so protocol errors can safely redirect.
            let redirect_uri = get_str("redirect_uri");
            let state = cs["state"].as_str().map(|s| s.to_string());
            tracing::warn!(error = %e, "consent: begin_authorization rejected");
            return Ok(protocol_error_redirect(&redirect_uri, state.as_deref(), e));
        }
    };

    let redirect = sui_id_core::authorize::complete_authorization(
        &app.db,
        &app.clock,
        user_id,
        &auth_methods,
        accepted,
    )
    .await
    .map_err(HttpError::html)?;

    let mut url = redirect.redirect_uri;
    let sep = if url.contains('?') { '&' } else { '?' };
    url.push(sep);
    url.push_str("code=");
    url.push_str(&url_encode(&redirect.code));
    if let Some(s) = redirect.state {
        url.push_str("&state=");
        url.push_str(&url_encode(&s));
    }

    Ok(axum::response::Redirect::to(&url).into_response())
}

fn url_encode(s: &str) -> String {
    percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC).to_string()
}
