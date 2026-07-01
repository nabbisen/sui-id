//! RFC 7662 (OAuth Token Introspection) and RFC 7009 (OAuth Token
//! Revocation) use cases.
//!
//! These two RFCs are siblings: both let a client send a token to a
//! dedicated endpoint, both authenticate the client the same way,
//! both accept an optional `token_type_hint`, and both treat
//! "already-invalid" as a non-error case. Putting them in one module
//! keeps the small amount of shared logic (client-credential check,
//! token-shape detection) in one place.

use crate::errors::{CoreError, CoreResult};
use crate::time::SharedClock;
use crate::tokens::{verify_access_token, AccessTokenClaims};
use chrono::DateTime;
use sui_id_shared::{ids::ClientId, RawRefreshToken};
use sui_id_store::repos::{
    clients, refresh_tokens, revoked_access_tokens as deny_list, users,
};
use sui_id_store::Database;

/// What the introspection endpoint reports back.
///
/// Mirrors the shape RFC 7662 requires. When `active` is false, every
/// other field must be omitted — leaking metadata about an inactive
/// token would defeat the point of revocation.
#[derive(Debug, Clone)]
pub struct IntrospectionResponse {
    pub active: bool,
    pub scope: Option<String>,
    pub client_id: Option<String>,
    pub username: Option<String>,
    pub token_type: Option<&'static str>,
    pub exp: Option<i64>,
    pub iat: Option<i64>,
    pub sub: Option<String>,
    pub aud: Option<String>,
    pub iss: Option<String>,
    /// "access_token" or "refresh_token", filled when active.
    pub kind: Option<&'static str>,
}

impl IntrospectionResponse {
    pub fn inactive() -> Self {
        Self {
            active: false,
            scope: None,
            client_id: None,
            username: None,
            token_type: None,
            exp: None,
            iat: None,
            sub: None,
            aud: None,
            iss: None,
            kind: None,
        }
    }
}

/// Per RFC 7662 §2.1, only registered clients may introspect, and a
/// client should only see back its own tokens — otherwise it could
/// fish for valid tokens issued to other parties. We enforce this by
/// requiring `aud == authenticating_client`.
pub async fn introspect(
    db: &Database,
    clock: &SharedClock,
    authenticating_client: ClientId,
    token: &str,
    hint: Option<&str>,
) -> CoreResult<IntrospectionResponse> {
    // Try in the order suggested by the hint, then fall back to the
    // other type. Either branch returning Active wins; both inactive
    // means inactive.
    let order: &[&str] = match hint {
        Some("refresh_token") => &["refresh_token", "access_token"],
        _ => &["access_token", "refresh_token"],
    };
    for kind in order {
        let resp = match *kind {
            "access_token" => try_introspect_access(db, clock, authenticating_client, token).await,
            "refresh_token" => try_introspect_refresh(db, authenticating_client, token).await,
            _ => Ok(IntrospectionResponse::inactive()),
        }?;
        if resp.active {
            return Ok(resp);
        }
    }
    Ok(IntrospectionResponse::inactive())
}

async fn try_introspect_access(
    db: &Database,
    clock: &SharedClock,
    authenticating_client: ClientId,
    token: &str,
) -> CoreResult<IntrospectionResponse> {
    // Verify the JWT (signature + exp). A bad signature or an expired
    // token simply means inactive; we deliberately do not surface why.
    let claims = match verify_access_token(db, clock, token).await {
        Ok(c) => c,
        Err(_) => return Ok(IntrospectionResponse::inactive()),
    };
    // Audience must match the authenticating client.
    let aud_id = claims
        .aud
        .parse::<ClientId>()
        .ok()
        .ok_or(())
        .map_err(|_| CoreError::Internal)?;
    if aud_id != authenticating_client {
        // Don't tell the caller why — same reason RFC 7662 §2.2 lists
        // every "no" as just `active: false`.
        return Ok(IntrospectionResponse::inactive());
    }
    // Check the deny-list.
    if deny_list::is_revoked(db, &claims.jti).await? {
        return Ok(IntrospectionResponse::inactive());
    }
    Ok(active_from_access(&claims, db).await)
}

async fn active_from_access(claims: &AccessTokenClaims, db: &Database) -> IntrospectionResponse {
    // username is best-effort: if the user no longer exists (deleted)
    // we still report the token active until exp, but skip the
    // username field. RFC 7662 doesn't require username; it's a
    // courtesy.
    let username = {
        let uid_opt = claims.sub.parse::<sui_id_shared::ids::UserId>().ok();
        if let Some(uid) = uid_opt {
            users::get(db, uid).await.ok().map(|u| u.username)
        } else {
            None
        }
    };
    IntrospectionResponse {
        active: true,
        scope: Some(claims.scope.clone()),
        client_id: Some(claims.aud.clone()),
        username,
        token_type: Some("Bearer"),
        exp: Some(claims.exp),
        iat: Some(claims.iat),
        sub: Some(claims.sub.clone()),
        aud: Some(claims.aud.clone()),
        iss: Some(claims.iss.clone()),
        kind: Some("access_token"),
    }
}

async fn try_introspect_refresh(
    db: &Database,
    authenticating_client: ClientId,
    token: &str,
) -> CoreResult<IntrospectionResponse> {
    let raw = RawRefreshToken::from_untrusted(token.to_owned());
    let row = match refresh_tokens::find_active(db, &raw).await {
        Ok(r) => r,
        Err(_) => return Ok(IntrospectionResponse::inactive()),
    };
    if row.client_id != authenticating_client {
        return Ok(IntrospectionResponse::inactive());
    }
    let username = users::get(db, row.user_id).await.ok().map(|u| u.username);
    Ok(IntrospectionResponse {
        active: true,
        scope: Some(row.scope),
        client_id: Some(row.client_id.to_string()),
        username,
        token_type: Some("Bearer"),
        exp: Some(row.expires_at.timestamp()),
        iat: Some(row.created_at.timestamp()),
        sub: Some(row.user_id.to_string()),
        aud: Some(row.client_id.to_string()),
        iss: None,
        kind: Some("refresh_token"),
    })
}

/// Per RFC 7009 §2.2, token revocation must be idempotent: revoking
/// an already-revoked or never-existed token returns 200. Only an
/// "unsupported token type" or auth failure produces an error.
pub async fn revoke(
    db: &Database,
    clock: &SharedClock,
    authenticating_client: ClientId,
    token: &str,
    hint: Option<&str>,
) -> CoreResult<()> {
    let order: &[&str] = match hint {
        Some("refresh_token") => &["refresh_token", "access_token"],
        _ => &["access_token", "refresh_token"],
    };
    for kind in order {
        let revoked = match *kind {
            "access_token" => try_revoke_access(db, clock, authenticating_client, token).await?,
            "refresh_token" => try_revoke_refresh(db, authenticating_client, token).await?,
            _ => false,
        };
        if revoked {
            return Ok(());
        }
    }
    // RFC 7009 §2.2: "The authorization server responds with HTTP
    // status code 200 if the token has been revoked successfully or
    // if the client submitted an invalid token." Either way, success.
    Ok(())
}

async fn try_revoke_access(
    db: &Database,
    clock: &SharedClock,
    authenticating_client: ClientId,
    token: &str,
) -> CoreResult<bool> {
    let claims = match verify_access_token(db, clock, token).await {
        Ok(c) => c,
        Err(_) => return Ok(false),
    };
    let aud_id = match claims.aud.parse::<ClientId>() {
        Ok(c) => c,
        Err(_) => return Ok(false),
    };
    if aud_id != authenticating_client {
        // Per RFC 7009 the right thing to do here is also "200 OK,
        // pretend it worked" rather than tell the client "that's not
        // your token". The client did supply a token whose audience
        // is somebody else; the safest is to do nothing and not leak.
        return Ok(false);
    }
    let exp = DateTime::from_timestamp(claims.exp, 0).ok_or(CoreError::Internal)?;
    let user_id = claims.sub.parse::<sui_id_shared::ids::UserId>().ok();
    deny_list::insert(
        db,
        &sui_id_store::repos::revoked_access_tokens::RevokedAccessTokenRow {
            jti: claims.jti.clone(),
            revoked_at: clock.now(),
            exp,
            revoked_by_user: user_id,
            revoked_by_client: Some(authenticating_client),
        },
    ).await?;
    Ok(true)
}

async fn try_revoke_refresh(
    db: &Database,
    authenticating_client: ClientId,
    token: &str,
) -> CoreResult<bool> {
    let raw = RawRefreshToken::from_untrusted(token.to_owned());
    let row = match refresh_tokens::find_active(db, &raw).await {
        Ok(r) => r,
        Err(_) => return Ok(false),
    };
    if row.client_id != authenticating_client {
        return Ok(false);
    }
    refresh_tokens::revoke(db, &row.id).await?;
    Ok(true)
}

/// Authenticate an introspection / revocation request from a
/// confidential client. Returns the client id on success.
///
/// We accept the credentials in either the form-body (`client_id` +
/// `client_secret`) or HTTP Basic — both are spec-permitted and the
/// HTTP layer normalises them before calling us.
pub async fn authenticate_client(
    db: &Database,
    client_id: &str,
    client_secret: &str,
) -> CoreResult<ClientId> {
    let id = client_id
        .parse::<ClientId>()
        .map_err(|_| CoreError::Unauthenticated)?;
    let row = clients::get(db, id).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::Unauthenticated,
        other => CoreError::from(other),
    })?;
    if !row.confidential {
        // RFC 7009 / 7662: public clients aren't supported at these
        // endpoints. They have no secret to authenticate with.
        return Err(CoreError::Unauthenticated);
    }
    if row.is_disabled || row.is_deleted {
        return Err(CoreError::Unauthenticated);
    }
    let hash = row
        .secret_hash
        .as_deref()
        .ok_or(CoreError::Unauthenticated)?;
    crate::password::verify_password(client_secret, hash)
        .map_err(|_| CoreError::Unauthenticated)?;
    Ok(id)
}
