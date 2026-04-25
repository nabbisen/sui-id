//! OIDC and OAuth 2.0 protocol endpoints.

use crate::errors::HttpError;
use crate::handlers::AppStateExt;
use axum::extract::Query;
use axum::http::{header, HeaderMap};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum::Json;
use base64ct::{Base64, Encoding};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use sui_id_core::authorize::{
    self, AuthorizeParams, CodeExchangeRequest, IssuanceContext, RefreshExchangeRequest,
};
use sui_id_core::discovery::Discovery;
use sui_id_core::errors::{CoreError, ProtocolError};
use sui_id_core::jwks;
use sui_id_shared::ids::ClientId;
use sui_id_store::repos::users;

// ---------- discovery & JWKS ----------

pub async fn discovery(state_ext: AppStateExt) -> impl IntoResponse {
    let axum::extract::State(app) = state_ext;
    Json(Discovery::build(app.issuer()))
}

pub async fn jwks(state_ext: AppStateExt) -> Result<Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let body = jwks::build(&app.db).map_err(|e| HttpError::api(CoreError::from(e)))?;
    let mut resp = Json(body).into_response();
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, "public, max-age=300".parse().unwrap_or_else(|_| "no-store".parse().expect("static")));
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

/// `GET /oauth2/authorize`. If the user is not authenticated, redirect them
/// to the login page with a `next` parameter pointing back here. Otherwise
/// validate the request and immediately issue an authorization code.
///
/// (sui-id deliberately does not show a separate "consent" screen in the
/// minimal version — see the spec's §11.5/§13.2 commentary.)
pub async fn authorize(
    state_ext: AppStateExt,
    Query(q): Query<AuthorizeQuery>,
    headers: HeaderMap,
) -> Result<Response, HttpError> {
    let axum::extract::State(app) = state_ext;

    // Resolve a session if one exists; otherwise redirect to login.
    let session_user = match resolve_session(&app, &headers).await {
        Some(uid) => uid,
        None => {
            let qs = build_query_string(&q);
            let next = format!("/oauth2/authorize?{qs}");
            let encoded = utf8_percent_encode(&next, NON_ALPHANUMERIC).to_string();
            return Ok(Redirect::to(&format!("/admin/login?next={encoded}")).into_response());
        }
    };

    let client_id = ClientId::from_str(&q.client_id).map_err(|_| {
        HttpError::html(CoreError::Protocol {
            code: ProtocolError::InvalidClient,
            description: "client_id is not a valid identifier".into(),
        })
    })?;

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

    let accepted = authorize::begin_authorization(&app.db, params).map_err(HttpError::html)?;
    let redirect = authorize::complete_authorization(&app.db, &app.clock, session_user, accepted)
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
) -> Option<sui_id_shared::ids::UserId> {
    use axum_extra::extract::cookie::CookieJar;
    use sui_id_shared::ids::SessionId;
    let jar = CookieJar::from_headers(headers);
    let raw = jar.get(crate::handlers::SESSION_COOKIE)?.value().to_string();
    let id = SessionId::from_str(&raw).ok()?;
    sui_id_core::session::resolve(&app.db, &app.clock, id).ok()
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
        crate::handlers::ErrorAs::Json,
    )?;

    let basic = parse_basic_auth(&headers);
    let (header_client, header_secret) = match basic {
        Some((c, s)) => (Some(c), Some(s)),
        None => (None, None),
    };
    let client_id_raw = header_client
        .or_else(|| form.client_id.clone())
        .ok_or_else(|| {
            HttpError::api(CoreError::Protocol {
                code: ProtocolError::InvalidClient,
                description: "client_id is required".into(),
            })
        })?;
    let client_id = ClientId::from_str(&client_id_raw).map_err(|_| {
        HttpError::api(CoreError::Protocol {
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
                HttpError::api(CoreError::Protocol {
                    code: ProtocolError::InvalidRequest,
                    description: "code is required".into(),
                })
            })?;
            let redirect_uri = form.redirect_uri.ok_or_else(|| {
                HttpError::api(CoreError::Protocol {
                    code: ProtocolError::InvalidRequest,
                    description: "redirect_uri is required".into(),
                })
            })?;
            let code_verifier = form.code_verifier.ok_or_else(|| {
                HttpError::api(CoreError::Protocol {
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
            .map_err(HttpError::api)?
        }
        "refresh_token" => {
            let refresh_token = form.refresh_token.ok_or_else(|| {
                HttpError::api(CoreError::Protocol {
                    code: ProtocolError::InvalidRequest,
                    description: "refresh_token is required".into(),
                })
            })?;
            authorize::exchange_refresh(
                &app.db,
                &app.clock,
                ctx,
                RefreshExchangeRequest {
                    refresh_token,
                    client_id,
                    client_secret,
                },
            )
            .map_err(HttpError::api)?
        }
        other => {
            return Err(HttpError::api(CoreError::Protocol {
                code: ProtocolError::UnsupportedGrantType,
                description: format!("unsupported grant_type: {other}"),
            }));
        }
    };

    let resp = TokenResponse {
        access_token: set.access_token,
        token_type: "Bearer",
        expires_in: set.access_expires_in,
        refresh_token: set.refresh_token,
        id_token: set.id_token,
        scope: None,
    };
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

fn parse_basic_auth(headers: &HeaderMap) -> Option<(String, String)> {
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
}

/// Standard OIDC userinfo endpoint. Authenticates via the Bearer access
/// token: we verify the JWT signature against our own JWKS, check expiry,
/// and look up the subject.
pub async fn userinfo(
    state_ext: AppStateExt,
    headers: HeaderMap,
) -> Result<Response, HttpError> {
    let axum::extract::State(app) = state_ext;
    let raw = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| HttpError::api(CoreError::Unauthenticated))?;

    let claims = sui_id_core::tokens::verify_access_token(&app.db, &app.clock, raw)
        .map_err(HttpError::api)?;
    let uid: sui_id_shared::ids::UserId = claims
        .sub
        .parse()
        .map_err(|_| HttpError::api(CoreError::Internal))?;
    let row = users::get(&app.db, uid).map_err(|_| HttpError::api(CoreError::Unauthenticated))?;
    if row.is_disabled || row.is_deleted {
        return Err(HttpError::api(CoreError::Unauthenticated));
    }
    Ok(Json(UserInfo {
        sub: row.id.to_string(),
        preferred_username: Some(row.username),
        name: row.display_name,
    })
    .into_response())
}
