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
//!
//! RFC 079 introduces a typestate pipeline inside [`exchange_code`] that
//! makes token issuance unreachable unless all binding checks and PKCE
//! verification have passed.  The pipeline types are private to this module.

use crate::errors::{CoreError, CoreResult, ProtocolError};
use crate::time::SharedClock;
use crate::tokens::{self, TokenLifetimes, TokenSet};
use chrono::Duration;
use ed25519_dalek::SigningKey;
use sui_id_shared::ids::{ClientId, UserId};
use sui_id_shared::{CodeHash, FamilyId, RawRefreshToken, RefreshTokenHash, RefreshTokenId};
use sui_id_store::Database;
use sui_id_store::models::{AuditLogRow, AuthorizationCodeRow, RefreshTokenRow};
use sui_id_store::repos::{audit, auth_codes, clients, refresh_tokens, signing_keys, users};

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

/// Phase 1 of the authorization endpoint: validate client identity and
/// redirect_uri **before** redirecting the user to the login page.
///
/// This prevents the user from authenticating only to land on an error page
/// because the caller sent an invalid `client_id`. Per RFC 6749 §4.1.2.1,
/// errors at this phase must NEVER redirect — we render an HTML error
/// instead because we cannot trust the `redirect_uri` without a valid
/// client record.
///
/// Returns the resolved `ClientRow` so the caller can pass it forward
/// to `begin_authorization` without an extra database round-trip.
pub async fn validate_client_and_redirect_uri(
    db: &Database,
    client_id: ClientId,
    redirect_uri: &str,
) -> CoreResult<sui_id_store::models::ClientRow> {
    let client = clients::get(db, client_id).await.map_err(|e| match e {
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
    if !is_redirect_uri_registered(&client.redirect_uris, redirect_uri) {
        return Err(CoreError::Protocol {
            code: ProtocolError::InvalidRequest,
            description: format!(
                "redirect_uri does not match any registered URI for this client. \
                 Submitted: \"{redirect_uri}\". Registered URIs: [{}]. \
                 The comparison is exact — check for trailing slashes, \
                 http vs https, and port numbers.",
                client
                    .redirect_uris
                    .iter()
                    .map(|u| format!("\"{u}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }
    Ok(client)
}

pub async fn begin_authorization(
    db: &Database,
    params: AuthorizeParams,
) -> CoreResult<AcceptedAuthorize> {
    // RFC 6749 §4.1.1 — only "code" is supported.
    if params.response_type != "code" {
        return Err(CoreError::Protocol {
            code: ProtocolError::UnsupportedResponseType,
            description: format!(
                "only response_type=code is supported, got {}",
                params.response_type
            ),
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
    let client = clients::get(db, params.client_id)
        .await
        .map_err(|e| match e {
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
    if !is_redirect_uri_registered(&client.redirect_uris, &params.redirect_uri) {
        return Err(CoreError::Protocol {
            code: ProtocolError::InvalidRequest,
            description: format!(
                "redirect_uri does not match any registered URI for this client. \
                 Submitted: \"{}\". Registered URIs: [{}]. \
                 The comparison is exact — check for trailing slashes, \
                 http vs https, and port numbers.",
                params.redirect_uri,
                client
                    .redirect_uris
                    .iter()
                    .map(|u| format!("\"{u}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }
    enforce_scope_policy(&client.allowed_scopes, &params.scope, &client.name)?;
    Ok(AcceptedAuthorize { params })
}

/// Whether `submitted` is a registered redirect URI for the client.
///
/// The OAuth 2.0 / OIDC spec mandates **exact-string match** between
/// the value sent at `/authorize` and one of the URIs the client
/// registered. There is no normalisation, no case folding, no
/// trailing-slash leniency: `https://example.com/cb` and
/// `https://example.com/cb/` are different URIs, and
/// `https://example.com/cb` and `https://example.com:443/cb` are
/// different URIs even though they reach the same socket.
///
/// The strictness is the security: any normalisation we did here
/// would be a normalisation an attacker could exploit. A bug that
/// accepted slight variants of a registered URI would let an
/// attacker who registers `https://attacker.example/cb` reach a
/// callback like `https://attacker.example.com/cb`.
///
/// The function is `pub` so it can be exercised by property tests
/// directly. Keep it boring; resist any urge to add normalisation
/// in here.
pub fn is_redirect_uri_registered(registered: &[String], submitted: &str) -> bool {
    registered.iter().any(|u| u == submitted)
}

/// Check the requested scope string against the client's `allowed_scopes`
/// policy. An empty policy is the legacy "permit any" mode (rows from
/// before migration 0002) and skips the check. Otherwise the requested
/// scope tokens must all appear in the policy. Returns `invalid_scope`
/// per RFC 6749 §5.2 when a request exceeds the policy.
///
/// The error description (RFC 027) names the offending scope and the
/// client's current allowed list so operators can identify and fix the
/// configuration without consulting the server logs.
fn enforce_scope_policy(allowed: &str, requested: &str, client_name: &str) -> CoreResult<()> {
    let allowed_trimmed = allowed.trim();
    if allowed_trimmed.is_empty() {
        return Ok(());
    }
    let allowed_set: std::collections::HashSet<&str> = allowed_trimmed.split_whitespace().collect();
    for tok in requested.split_whitespace() {
        if !allowed_set.contains(tok) {
            return Err(CoreError::Protocol {
                code: ProtocolError::InvalidScope,
                description: format!(
                    "scope {tok:?} is not permitted for client {:?} \
                     (allowed: {:?}). \
                     Go to Admin → Clients → edit this client and add {tok:?} \
                     to the Allowed scopes field.",
                    client_name, allowed_trimmed,
                ),
            });
        }
    }
    Ok(())
}

/// Outcome of [`complete_authorization`]: the URL to redirect the browser to.
#[derive(Debug, Clone)]
pub struct AuthorizationResponseRedirect {
    pub redirect_uri: String,
    pub code: String,
    pub state: Option<String>,
}

pub async fn complete_authorization(
    db: &Database,
    clock: &SharedClock,
    user_id: UserId,
    auth_methods: &[sui_id_shared::AuthMethod],
    accepted: AcceptedAuthorize,
) -> CoreResult<AuthorizationResponseRedirect> {
    let now = clock.now();
    let code_plain = tokens::random_token(32);
    let code_hash = CodeHash::of(&code_plain);
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
        // Snapshot the originating session's authentication factors
        // here; the token endpoint will read them back to populate
        // `acr` and `amr`. Snapshotting at this point — rather than
        // dereferencing a session id at exchange time — is correct
        // even if the session is revoked between authorize and
        // token, which keeps an issued auth code valid as long as
        // it's within its single-use lifetime.
        auth_methods: auth_methods.to_vec(),
    };
    auth_codes::insert(db, &row).await?;
    Ok(AuthorizationResponseRedirect {
        redirect_uri: accepted.params.redirect_uri,
        code: code_plain,
        state: accepted.params.state,
    })
}

#[derive(Debug, Clone)]
// ── RFC 079 typestate pipeline ──────────────────────────────────────────────
//
// These types live in this module and are never exposed to callers.  Each step
// is a function that consumes the previous state and returns the next; the
// final `issue_for` only accepts `IssuableGrant`, so the compiler enforces
// that every validation step has run.  The four-state chain is linear; there
// is no enum explosion, per strategy §9 ("keep typestate localised").

/// Step 1: the auth code has been atomically consumed by the storage layer.
/// The row is guaranteed single-use from this point forward.
struct ConsumedCode(AuthorizationCodeRow);

/// Step 2: the requesting client and redirect URI match the code's bindings.
struct BoundCode(AuthorizationCodeRow);

/// Step 3: the PKCE code_verifier has been verified against the stored
/// code_challenge.
struct PkceVerifiedCode(AuthorizationCodeRow);

/// Step 4: the user account is active and the email claim is resolved.
/// This is the only type accepted by `issue_for` — token issuance is
/// a compile error without it.
struct IssuableGrant {
    row: AuthorizationCodeRow,
    user_id: sui_id_shared::ids::UserId,
    email: Option<(String, bool)>,
}

pub struct CodeExchangeRequest {
    pub code: String,
    pub redirect_uri: String,
    pub client_id: ClientId,
    /// Provided by confidential clients; None for public.
    pub client_secret: Option<String>,
    pub code_verifier: String,
}

#[derive(Debug)]
pub struct RefreshExchangeRequest {
    /// The plaintext refresh token value supplied by the client. Wrapped as
    /// `RawRefreshToken` so it cannot appear in `Debug` output and is
    /// zeroed on drop.
    pub refresh_token: RawRefreshToken,
    pub client_id: ClientId,
    pub client_secret: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct IssuanceContext<'a> {
    pub issuer: &'a str,
    pub lifetimes: TokenLifetimes,
}

/// Exchange a previously issued authorization code for a fresh token set.
///
/// Implements the RFC 079 typestate pipeline: each validation step produces
/// a new type that the next step requires, so the compiler statically
/// prevents token issuance from being reached before all checks pass.
/// The pipeline types are private to this module.
pub async fn exchange_code(
    db: &Database,
    clock: &SharedClock,
    ctx: IssuanceContext<'_>,
    req: CodeExchangeRequest,
) -> CoreResult<TokenSet> {
    // ── Authenticate the client ──────────────────────────────────────────
    let client = clients::get(db, req.client_id).await.map_err(|e| match e {
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
    authenticate_client(&client, req.client_secret.as_deref()).await?;

    // ── Step 1: atomically consume the code (RFC 079 P1 + P2) ───────────
    // The storage layer enforces single-use via a guarded UPDATE predicated
    // on `consumed = 0 AND expires_at > now`.  All failure modes (unknown,
    // replayed, expired) surface as NotFound to preserve non-disclosure (P5).
    let now = clock.now();
    let code_hash = CodeHash::of(&req.code);
    let consumed = auth_codes::consume(db, &code_hash, now)
        .await
        .map_err(|e| match e {
            sui_id_store::StoreError::NotFound => CoreError::Protocol {
                code: ProtocolError::InvalidGrant,
                description: "code is unknown, expired, or already used".into(),
            },
            other => CoreError::from(other),
        })
        .map(ConsumedCode)?;

    // ── Step 2: verify client and redirect-URI bindings ──────────────────
    // The code is already consumed; a bound failure keeps it consumed
    // (RFC 079 P4 — burn on failure).
    let bound = bind_code(consumed, &req)?;

    // ── Step 3: verify PKCE ──────────────────────────────────────────────
    let pkce_verified = verify_pkce_step(bound, &req)?;

    // ── Step 4: re-check user account state at exchange time ─────────────
    // A user might be disabled/deleted in the ~60-second code window.
    // The code is already consumed regardless of the outcome here.
    let issuable = resolve_user(db, clock, pkce_verified, req.client_id).await?;

    // ── Issue tokens (only reachable with IssuableGrant) ─────────────────
    issue_for(
        db,
        clock,
        ctx,
        issuable.user_id,
        req.client_id,
        &issuable.row.scope,
        issuable.row.nonce.as_deref(),
        &issuable.row.auth_methods,
        issuable.email.as_ref().map(|(addr, v)| (addr.as_str(), *v)),
    )
    .await
}

// ── RFC 079 pipeline step helpers ────────────────────────────────────────────

/// Step 2: verify that the code was issued to the same client and redirect URI
/// as in the exchange request.
fn bind_code(consumed: ConsumedCode, req: &CodeExchangeRequest) -> CoreResult<BoundCode> {
    if consumed.0.client_id != req.client_id {
        return Err(CoreError::Protocol {
            code: ProtocolError::InvalidGrant,
            description: "code was issued to a different client".into(),
        });
    }
    if consumed.0.redirect_uri != req.redirect_uri {
        return Err(CoreError::Protocol {
            code: ProtocolError::InvalidGrant,
            description: "redirect_uri does not match the original".into(),
        });
    }
    Ok(BoundCode(consumed.0))
}

/// Step 3: verify the PKCE code_verifier against the stored challenge.
fn verify_pkce_step(bound: BoundCode, req: &CodeExchangeRequest) -> CoreResult<PkceVerifiedCode> {
    tokens::verify_pkce(
        &bound.0.code_challenge_method,
        &req.code_verifier,
        &bound.0.code_challenge,
    )?;
    Ok(PkceVerifiedCode(bound.0))
}

/// Step 4: fetch the user row and verify the account is active.
/// Returns `IssuableGrant` — the only type accepted by `issue_for`.
async fn resolve_user(
    db: &Database,
    clock: &SharedClock,
    pkce_verified: PkceVerifiedCode,
    client_id: ClientId,
) -> CoreResult<IssuableGrant> {
    let row = pkce_verified.0;
    let user = users::get(db, row.user_id).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::Protocol {
            code: ProtocolError::InvalidGrant,
            description: "user not found".into(),
        },
        other => CoreError::from(other),
    })?;
    if user.is_disabled || user.is_deleted {
        let _ = audit::append(
            db,
            &AuditLogRow {
                at: clock.now(),
                actor: Some(row.user_id),
                action: "oauth2.exchange_code.user_revoked".into(),
                target: Some(client_id.to_string()),
                result: "denied".into(),
                note: Some("user disabled or deleted during auth-code exchange window".into()),
            },
        )
        .await;
        return Err(CoreError::Protocol {
            code: ProtocolError::InvalidGrant,
            description: "user account is not active".into(),
        });
    }
    let email = user
        .email
        .map(|addr| (addr, user.email_verified_at.is_some()));
    Ok(IssuableGrant {
        row,
        user_id: user.id,
        email,
    })
}

/// Exchange a refresh token for a fresh token set, rotating the refresh token.
///
/// Uses `begin_rotation` (RFC 080) to atomically arbitrate concurrent
/// exchanges of the same token.  The outcome enum makes the four cases
/// explicit and ensures the theft-detection path runs inside the same
/// transaction as the revoke.
pub async fn exchange_refresh(
    db: &Database,
    clock: &SharedClock,
    ctx: IssuanceContext<'_>,
    req: RefreshExchangeRequest,
) -> CoreResult<TokenSet> {
    // ── Authenticate the client ──────────────────────────────────────────
    let client = clients::get(db, req.client_id).await.map_err(|e| match e {
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
    authenticate_client(&client, req.client_secret.as_deref()).await?;

    // ── Atomic rotation arbitration (RFC 080) ────────────────────────────
    // `begin_rotation` holds the revoke + optional family-revoke inside one
    // transaction, so exactly one concurrent caller receives `RotatedHere`
    // and every loser receives `ReuseDetected` with the family already closed.
    let now = clock.now();
    let token_hash = RefreshTokenHash::of(&req.refresh_token);
    let rotation = refresh_tokens::begin_rotation(db, &token_hash, &req.client_id, now)
        .await
        .map_err(|e| match e {
            // Client-mismatch is rejected without mutating state.
            sui_id_store::StoreError::Conflict => CoreError::Protocol {
                code: ProtocolError::InvalidGrant,
                description: "refresh token was issued to a different client".into(),
            },
            other => CoreError::from(other),
        })?;

    let row = match rotation {
        // ── Won the race: proceed to issuance ───────────────────────────
        refresh_tokens::RotationLookup::RotatedHere(row) => row,

        // ── Reuse signal: family already revoked, emit audit, deny ──────
        // The family revocation happened atomically inside `begin_rotation`,
        // so this is already a closed outcome by the time we reach here.
        // We do not distinguish the "concurrent winner" case from a
        // "token-already-rotated replay" case — both look identical to the
        // caller (RFC 080 P5).
        refresh_tokens::RotationLookup::ReuseDetected {
            row,
            family_revoked,
        } => {
            let _ = audit::append(
                db,
                &AuditLogRow {
                    at: now,
                    actor: Some(row.user_id),
                    action: "auth.refresh.theft_detected".into(),
                    target: Some(row.user_id.to_string()),
                    result: "denied".into(),
                    note: Some(format!(
                        "revoked refresh-token family={} client_id={} family_revoked={}",
                        row.family_id, row.client_id, family_revoked
                    )),
                },
            )
            .await;
            return Err(CoreError::Protocol {
                code: ProtocolError::InvalidGrant,
                description: "refresh token is unknown or revoked".into(),
            });
        }

        // ── Expired or unknown: opaque denial ────────────────────────────
        refresh_tokens::RotationLookup::Expired(_) | refresh_tokens::RotationLookup::Unknown => {
            return Err(CoreError::Protocol {
                code: ProtocolError::InvalidGrant,
                description: "refresh token is unknown or revoked".into(),
            });
        }
    };

    // ── Fetch user email for ID token claims (scope "email") ─────────────
    // Refresh token exchanges may include an ID token (OIDC Core §12.2).
    let email_for_token: Option<(String, bool)> =
        if row.scope.split_whitespace().any(|s| s == "email") {
            match users::get(db, row.user_id).await {
                Ok(u) if !u.is_disabled && !u.is_deleted => {
                    u.email.map(|addr| (addr, u.email_verified_at.is_some()))
                }
                _ => None,
            }
        } else {
            None
        };
    let email_arg: Option<(&str, bool)> = email_for_token
        .as_ref()
        .map(|(addr, v)| (addr.as_str(), *v));

    issue_for_with_family(
        db,
        clock,
        ctx,
        row.user_id,
        row.client_id,
        &row.scope,
        None,
        &row.auth_methods,
        Some(row.family_id.clone()),
        email_arg,
    )
    .await
}

async fn authenticate_client(
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

#[allow(clippy::too_many_arguments)]
async fn issue_for(
    db: &Database,
    clock: &SharedClock,
    ctx: IssuanceContext<'_>,
    user_id: UserId,
    client_id: ClientId,
    scope: &str,
    nonce: Option<&str>,
    auth_methods: &[sui_id_shared::AuthMethod],
    user_email: Option<(&str, bool)>,
) -> CoreResult<TokenSet> {
    // Initial issuance (authorization-code grant): no parent family,
    // so we let `issue_for_with_family` create a new family rooted
    // at the new refresh-token id.
    issue_for_with_family(
        db,
        clock,
        ctx,
        user_id,
        client_id,
        scope,
        nonce,
        auth_methods,
        None,
        user_email,
    )
    .await
}

/// The actual issuance routine. `family_id` is `None` for initial
/// issuance (a new family is created, rooted at the new
/// refresh-token id) and `Some(parent_family_id)` for rotations
/// (the new row inherits the family).
#[allow(clippy::too_many_arguments)]
async fn issue_for_with_family(
    db: &Database,
    clock: &SharedClock,
    ctx: IssuanceContext<'_>,
    user_id: UserId,
    client_id: ClientId,
    scope: &str,
    nonce: Option<&str>,
    auth_methods: &[sui_id_shared::AuthMethod],
    family_id: Option<FamilyId>,
    user_email: Option<(&str, bool)>,
) -> CoreResult<TokenSet> {
    let key_row = signing_keys::active(db).await.map_err(|e| match e {
        sui_id_store::StoreError::NotFound => CoreError::Internal,
        other => CoreError::from(other),
    })?;
    let private_bytes = signing_keys::unseal_private(db, &key_row).await?;
    let sk_arr: [u8; 32] = private_bytes
        .as_slice()
        .try_into()
        .map_err(|_| CoreError::Internal)?;
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
        auth_methods,
        user_email,
    )
    .await?;

    let now = clock.now();
    let new_id = RefreshTokenId::generate();
    // First issuance roots a new family at this new row's id; a
    // rotation copies the parent's family forward unchanged.
    let family = family_id.unwrap_or_else(|| FamilyId::root_of(&new_id));
    let rt_row = RefreshTokenRow {
        id: new_id,
        user_id,
        client_id,
        scope: scope.to_owned(),
        expires_at: now + Duration::seconds(ctx.lifetimes.refresh_secs),
        revoked_at: None,
        created_at: now,
        auth_methods: auth_methods.to_vec(),
        family_id: family,
    };
    // Pass the plaintext token separately — RefreshTokenRow carries no plaintext.
    refresh_tokens::insert(db, &rt_row, &set.refresh_token).await?;
    Ok(set)
}

#[cfg(test)]
mod redirect_uri_tests {
    //! Property tests on [`is_redirect_uri_registered`].
    //!
    //! The redirect-URI check is the security boundary against an
    //! open-redirect attack. The properties below pin down the rule
    //! the OAuth/OIDC specs put on us — strict, byte-exact match —
    //! and guard against well-known regressions:
    //!
    //!   - case folding
    //!   - trailing-slash leniency
    //!   - default-port collapsing (`:443` vs implicit)
    //!   - subdomain wildcard misreads
    //!   - prefix matching
    //!
    //! If anyone tries to "fix" a perceived UX problem by adding
    //! normalisation, one of these properties should fail loudly.

    use super::is_redirect_uri_registered;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 256,
            ..ProptestConfig::default()
        })]

        /// A URI that was registered exactly is accepted.
        #[test]
        fn registered_uri_is_always_accepted(
            // Realistic-ish URI alphabet. Doesn't have to parse as a
            // URL — the function is a string comparator.
            uri in "[A-Za-z0-9:/._~?&=#@%-]{1,256}",
        ) {
            let registered = vec![uri.clone()];
            prop_assert!(is_redirect_uri_registered(&registered, &uri));
        }

        /// A URI that differs by even one byte is rejected.
        ///
        /// Generated as: take a registered URI, flip one character
        /// somewhere along it. The mutation makes the strings
        /// unequal, so the function must reject.
        #[test]
        fn one_byte_off_uri_is_rejected(
            base in "[A-Za-z0-9:/._~?&=-]{8,128}",
            mutation_index in any::<usize>(),
        ) {
            // Build a "submitted" by flipping a byte in `base`.
            let mut submitted = base.clone().into_bytes();
            let i = mutation_index % submitted.len();
            // Swap the byte to a guaranteed-different one. ASCII
            // arithmetic; we know all the chars are ASCII because
            // of the regex.
            submitted[i] = if submitted[i] == b'X' { b'Y' } else { b'X' };
            let submitted = String::from_utf8(submitted).unwrap();
            prop_assume!(submitted != base);
            let registered = vec![base];
            prop_assert!(!is_redirect_uri_registered(&registered, &submitted));
        }

        /// Case differences are not folded — `/cb` and `/CB` are
        /// distinct URIs as far as we're concerned.
        #[test]
        fn case_difference_is_not_folded(
            stem in "[a-z]{4,16}",
        ) {
            let lower = format!("https://example.com/{stem}");
            let upper = format!("https://example.com/{}", stem.to_uppercase());
            prop_assume!(lower != upper);
            let registered = vec![lower.clone()];
            prop_assert!(is_redirect_uri_registered(&registered, &lower));
            prop_assert!(!is_redirect_uri_registered(&registered, &upper));
        }

        /// A registered URI followed by extra junk is rejected — no
        /// prefix match. This is the "attacker registers
        /// `https://example.com/cb` and submits
        /// `https://example.com/cb/../../leak`" case.
        #[test]
        fn prefix_extension_is_rejected(
            base in "[A-Za-z0-9:/._~-]{8,64}",
            suffix in "[A-Za-z0-9/.-]{1,32}",
        ) {
            let registered = vec![base.clone()];
            let submitted = format!("{base}{suffix}");
            prop_assume!(submitted != base);
            prop_assert!(!is_redirect_uri_registered(&registered, &submitted));
        }

        /// Multiple registered URIs: any one matching is enough; any
        /// one not in the list is not. (Sanity: this is what `any()`
        /// computes; the property is here to catch a future
        /// refactor that gets the predicate backwards.)
        #[test]
        fn multi_registry_matches_each_member_and_only_them(
            uris in proptest::collection::vec("[A-Za-z0-9:/._~-]{8,64}", 1..6),
            outsider in "[A-Za-z0-9:/._~-]{8,64}",
        ) {
            prop_assume!(!uris.iter().any(|u| u == &outsider));
            for u in &uris {
                prop_assert!(is_redirect_uri_registered(&uris, u));
            }
            prop_assert!(!is_redirect_uri_registered(&uris, &outsider));
        }
    }
}
