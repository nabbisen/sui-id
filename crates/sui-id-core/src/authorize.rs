//! Authorization Code + PKCE flow.
//!
//! Two entry points:
//!
//! 1. [`begin_authorization`]: validates the incoming `/authorize` request
//!    (client, redirect URI, response type, PKCE params, scopes) and returns
//!    the parsed parameters that the caller should remember while the user
//!    interacts with the login form.
//!
//! 2. [`complete_authorization`]: after the user has authenticated, persists
//!    a single-use authorization code and returns the redirect target.
//!
//! Token exchange ([`exchange_code`]) and refresh ([`exchange_refresh`]) live
//! here as well: they consume the code or refresh token, run all the standard
//! verifications (PKCE, redirect URI match, expiry, single-use) and emit a
//! fresh token set. Refresh tokens are *rotated* — the previous one is
//! revoked at the same time the new one is issued.

use crate::errors::{CoreError, CoreResult, ProtocolError};
use crate::time::SharedClock;
use crate::tokens::{self, TokenLifetimes, TokenSet};
use chrono::Duration;
use ed25519_dalek::SigningKey;
use sui_id_shared::ids::{ClientId, UserId};
use sui_id_store::models::{AuthorizationCodeRow, RefreshTokenRow};
use sui_id_store::repos::{auth_codes, clients, refresh_tokens, signing_keys};
use sui_id_store::Database;

/// Lifetime of an authorization code (kept very short by design).
const AUTH_CODE_LIFETIME_SECS: i64 = 60;

/// Parsed request to /authorize.
#[derive(Debug, Clone)]
pub struct AuthorizeParams {
    pub client_id: ClientId,
    pub redirect_uri: String,
    pub response_type: String,
    pub scope: String,
    pub state: Option<String>,
    pub nonce: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: String,
}

/// Result of validating /authorize: the request is well-formed and the
/// caller may now show the login UI.
#[derive(Debug, Clone)]
pub struct AcceptedAuthorize {
    pub params: AuthorizeParams,
}

pub fn begin_authorization(db: &Database, params: AuthorizeParams) -> CoreResult<AcceptedAuthorize> {
    // RFC 6749 §4.1.1 — only "code" is supported.
    if params.response_type != "code" {
        return Err(CoreError::Protocol {
            code: ProtocolError::UnsupportedResponseType,
            description: format!("only response_type=code is supported, got {}", params.response_type),
        });
    }
    // PKCE is mandatory.
    if params.code_challenge.is_empty() {
        return Err(CoreError::Protocol {
            code: ProtocolError::InvalidRequest,
            description: "code_challenge is required (PKCE)".into(),
        });
    }
    if params.code_challenge_method != "S256" {
        return Err(CoreError::Protocol {
            code: ProtocolError::InvalidRequest,
            description: "code_challenge_method must be S256".into(),
        });
    }
    let client = clients::get(db, params.client_id).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::Protocol {
            code: ProtocolError::InvalidClient,
            description: "unknown client_id".into(),
        },
        other => CoreError::from(other),
    })?;
    if client.is_disabled || client.is_deleted {
        return Err(CoreError::Protocol {
            code: ProtocolError::UnauthorizedClient,
            description: "client is not allowed to use the authorization endpoint".into(),
        });
    }
    if !client.redirect_uris.iter().any(|u| u == &params.redirect_uri) {
        return Err(CoreError::Protocol {
            code: ProtocolError::InvalidRequest,
            description: "redirect_uri does not match a registered URI".into(),
        });
    }
    Ok(AcceptedAuthorize { params })
}

/// Outcome of [`complete_authorization`]: the URL to redirect the browser to.
#[derive(Debug, Clone)]
pub struct AuthorizationResponseRedirect {
    pub redirect_uri: String,
    pub code: String,
    pub state: Option<String>,
}

pub fn complete_authorization(
    db: &Database,
    clock: &SharedClock,
    user_id: UserId,
    accepted: AcceptedAuthorize,
) -> CoreResult<AuthorizationResponseRedirect> {
    let now = clock.now();
    let code_plain = tokens::random_token(32);
    let code_hash = tokens::sha256_hex(&code_plain);
    let row = AuthorizationCodeRow {
        code_hash,
        client_id: accepted.params.client_id,
        user_id,
        redirect_uri: accepted.params.redirect_uri.clone(),
        scope: accepted.params.scope.clone(),
        nonce: accepted.params.nonce.clone(),
        code_challenge: accepted.params.code_challenge.clone(),
        code_challenge_method: accepted.params.code_challenge_method.clone(),
        expires_at: now + Duration::seconds(AUTH_CODE_LIFETIME_SECS),
        consumed: false,
        created_at: now,
    };
    auth_codes::insert(db, &row)?;
    Ok(AuthorizationResponseRedirect {
        redirect_uri: accepted.params.redirect_uri,
        code: code_plain,
        state: accepted.params.state,
    })
}

#[derive(Debug, Clone)]
pub struct CodeExchangeRequest {
    pub code: String,
    pub redirect_uri: String,
    pub client_id: ClientId,
    /// Provided by confidential clients; None for public.
    pub client_secret: Option<String>,
    pub code_verifier: String,
}

#[derive(Debug, Clone)]
pub struct RefreshExchangeRequest {
    pub refresh_token: String,
    pub client_id: ClientId,
    pub client_secret: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct IssuanceContext<'a> {
    pub issuer: &'a str,
    pub lifetimes: TokenLifetimes,
}

/// Exchange a previously issued authorization code for a fresh token set.
pub fn exchange_code(
    db: &Database,
    clock: &SharedClock,
    ctx: IssuanceContext<'_>,
    req: CodeExchangeRequest,
) -> CoreResult<TokenSet> {
    let client = clients::get(db, req.client_id).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::Protocol {
            code: ProtocolError::InvalidClient,
            description: "unknown client".into(),
        },
        other => CoreError::from(other),
    })?;
    if client.is_disabled || client.is_deleted {
        return Err(CoreError::Protocol {
            code: ProtocolError::UnauthorizedClient,
            description: "client is not allowed".into(),
        });
    }
    authenticate_client(&client, req.client_secret.as_deref())?;

    let code_hash = tokens::sha256_hex(&req.code);
    let row = auth_codes::consume(db, &code_hash).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::Protocol {
            code: ProtocolError::InvalidGrant,
            description: "code is unknown, expired, or already used".into(),
        },
        other => CoreError::from(other),
    })?;

    if row.client_id != req.client_id {
        return Err(CoreError::Protocol {
            code: ProtocolError::InvalidGrant,
            description: "code was issued to a different client".into(),
        });
    }
    if row.redirect_uri != req.redirect_uri {
        return Err(CoreError::Protocol {
            code: ProtocolError::InvalidGrant,
            description: "redirect_uri does not match the original".into(),
        });
    }
    tokens::verify_pkce(&row.code_challenge_method, &req.code_verifier, &row.code_challenge)?;

    issue_for(db, clock, ctx, row.user_id, req.client_id, &row.scope, row.nonce.as_deref())
}

/// Exchange a refresh token for a fresh token set, rotating the refresh token.
pub fn exchange_refresh(
    db: &Database,
    clock: &SharedClock,
    ctx: IssuanceContext<'_>,
    req: RefreshExchangeRequest,
) -> CoreResult<TokenSet> {
    let client = clients::get(db, req.client_id).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::Protocol {
            code: ProtocolError::InvalidClient,
            description: "unknown client".into(),
        },
        other => CoreError::from(other),
    })?;
    if client.is_disabled || client.is_deleted {
        return Err(CoreError::Protocol {
            code: ProtocolError::UnauthorizedClient,
            description: "client is not allowed".into(),
        });
    }
    authenticate_client(&client, req.client_secret.as_deref())?;

    let row = refresh_tokens::find_active(db, &req.refresh_token).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::Protocol {
            code: ProtocolError::InvalidGrant,
            description: "refresh token is unknown or revoked".into(),
        },
        other => CoreError::from(other),
    })?;

    if row.client_id != req.client_id {
        return Err(CoreError::Protocol {
            code: ProtocolError::InvalidGrant,
            description: "refresh token was issued to a different client".into(),
        });
    }

    // Rotation: revoke the old token *before* we issue the new set, so a
    // crash-mid-flow can never leave both valid simultaneously.
    refresh_tokens::revoke(db, &row.id)?;

    issue_for(db, clock, ctx, row.user_id, row.client_id, &row.scope, None)
}

fn authenticate_client(
    client: &sui_id_store::models::ClientRow,
    secret: Option<&str>,
) -> CoreResult<()> {
    if !client.confidential {
        return Ok(());
    }
    let stored = client.secret_hash.as_deref().ok_or(CoreError::Protocol {
        code: ProtocolError::InvalidClient,
        description: "client is confidential but has no stored secret".into(),
    })?;
    let provided = secret.ok_or(CoreError::Protocol {
        code: ProtocolError::InvalidClient,
        description: "client_secret is required".into(),
    })?;
    crate::password::verify_password(provided, stored).map_err(|_| CoreError::Protocol {
        code: ProtocolError::InvalidClient,
        description: "client authentication failed".into(),
    })
}

fn issue_for(
    db: &Database,
    clock: &SharedClock,
    ctx: IssuanceContext<'_>,
    user_id: UserId,
    client_id: ClientId,
    scope: &str,
    nonce: Option<&str>,
) -> CoreResult<TokenSet> {
    let key_row = signing_keys::active(db).map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::Internal,
        other => CoreError::from(other),
    })?;
    let private_bytes = signing_keys::unseal_private(db, &key_row)?;
    let sk_arr: [u8; 32] = private_bytes.as_slice().try_into().map_err(|_| CoreError::Internal)?;
    let sk = SigningKey::from_bytes(&sk_arr);

    let include_id_token = scope.split_whitespace().any(|s| s == "openid");
    let set = tokens::issue_token_set(
        ctx.issuer,
        user_id,
        client_id,
        scope,
        nonce,
        include_id_token,
        &key_row.id.to_string(),
        &sk,
        ctx.lifetimes,
        clock,
    )?;

    // Persist the refresh token (sealed at rest).
    let now = clock.now();
    let rt_row = RefreshTokenRow {
        id: tokens::random_token(16),
        token_plain: Some(set.refresh_token.clone()),
        user_id,
        client_id,
        scope: scope.to_owned(),
        expires_at: now + Duration::seconds(ctx.lifetimes.refresh_secs),
        revoked_at: None,
        created_at: now,
    };
    refresh_tokens::insert(db, &rt_row)?;
    Ok(set)
}
