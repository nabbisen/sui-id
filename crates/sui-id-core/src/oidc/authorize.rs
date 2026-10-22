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
use sui_id_store::models::{AuditLogRow, AuthorizationCodeRow, ClientRow, RefreshTokenRow};
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
    if !redirect_uri_matches(&client, redirect_uri) {
        return Err(CoreError::Protocol {
            code: ProtocolError::InvalidRequest,
            description: format!(
                "redirect_uri does not match any registered URI for this client. \
                 Submitted: \"{redirect_uri}\". Registered URIs: [{}]. \
                 The comparison is exact (port-flexible only for a \
                 PublicNativeLoopback client, RFC 8252) — check for \
                 trailing slashes, http vs https, and other port numbers.",
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
    if !redirect_uri_matches(&client, &params.redirect_uri) {
        return Err(CoreError::Protocol {
            code: ProtocolError::InvalidRequest,
            description: format!(
                "redirect_uri does not match any registered URI for this client. \
                 Submitted: \"{}\". Registered URIs: [{}]. \
                 The comparison is exact (port-flexible only for a \
                 PublicNativeLoopback client, RFC 8252) — check for \
                 trailing slashes, http vs https, and other port numbers.",
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

/// RFC 095 M3 stage 1b / RFC 8252: the one exception to exact matching, and
/// no other. For a `PublicNativeLoopback` client — public auth
/// (`!confidential`) with every registered redirect a numeric loopback
/// `http` address — the *port* may differ between registration and the
/// request, because the OS assigns an ephemeral port at launch and the
/// client cannot know it in advance. Scheme, host, path, query and
/// fragment still must match exactly; a difference in any of those is
/// still a rejection, same as [`is_redirect_uri_registered`].
///
/// `RedirectProfile` is never persisted (`sui-id`'s own
/// `dynamic_registration_validation` module owns that enum, at
/// registration time only) — this re-derives just enough of it from
/// `ClientRow.redirect_uris` and `ClientRow.confidential`, already stored,
/// per the handoff's explicit "no migration, no new column." A client
/// whose stored redirects are HTTPS can never reach the loopback branch at
/// all, so `ConfidentialHttps` and `PublicHttps` clients get exactly
/// [`is_redirect_uri_registered`]'s behaviour, unchanged.
///
/// Deliberately not used for `post_logout_redirect_uris` matching or CORS
/// origin comparison — RFC 095's own architecture document draws this
/// exception's scope at authorization-redirect matching for this one
/// profile and nowhere else; neither of those call sites calls this
/// function, and this function does not call `is_redirect_uri_registered`
/// in a way that would make it reachable from them by accident.
pub fn redirect_uri_matches(client: &ClientRow, submitted: &str) -> bool {
    if is_redirect_uri_registered(&client.redirect_uris, submitted) {
        return true;
    }
    if client.confidential {
        return false;
    }
    let Ok(submitted_url) = url::Url::parse(submitted) else {
        return false;
    };
    if !is_numeric_loopback_http(&submitted_url) {
        return false;
    }
    client
        .redirect_uris
        .iter()
        .all(|u| is_numeric_loopback_http_str(u))
        && client.redirect_uris.iter().any(|registered| {
            let Ok(registered_url) = url::Url::parse(registered) else {
                return false;
            };
            registered_url.host() == submitted_url.host()
                && registered_url.path() == submitted_url.path()
                && registered_url.query() == submitted_url.query()
                && registered_url.fragment() == submitted_url.fragment()
            // Port is deliberately excluded from this comparison -- the
            // one and only difference from `is_redirect_uri_registered`.
        })
}

fn is_numeric_loopback_http(url: &url::Url) -> bool {
    url.scheme() == "http"
        && (matches!(
            url.host(),
            Some(url::Host::Ipv4(ip)) if ip.is_loopback()
        ) || matches!(
            url.host(),
            Some(url::Host::Ipv6(ip)) if ip.is_loopback()
        ))
}

fn is_numeric_loopback_http_str(uri: &str) -> bool {
    url::Url::parse(uri).is_ok_and(|u| is_numeric_loopback_http(&u))
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

/// RFC 095 M3 stage 4b §2: which channel(s) a client credential arrived
/// on. A three-state value, not a `bool` (cannot represent `Both` at all)
/// and not two `bool`s (four representable states for three real ones,
/// with `(false, false)` duplicating a fact `client_secret: Option<_>`
/// already carries on the struct this travels with).
///
/// **This type says nothing about whether a credential arrived at all.**
/// That fact lives entirely in `client_secret`/`refresh_token`'s own
/// sibling field on the request struct: `None` there means "no credential
/// arrived," and this field's value is meaningless and must not be
/// consulted in that case. This type only ever distinguishes *which*
/// channel(s) carried one, given that one did.
///
/// The HTTP layer is the only place that can observe the channel
/// (`handlers/oidc.rs`'s own Basic/body extraction); core keeps the
/// decision of what to do with it, because that decision needs the
/// client's stored `token_endpoint_auth_method` and the HTTP layer has no
/// business loading client rows to authenticate a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialChannel {
    /// Arrived via the `Authorization: Basic` header only.
    Basic,
    /// Arrived in the request body (`client_secret` form field) only.
    Body,
    /// Arrived on both channels at once — RFC 6749 §2.3.1 forbids
    /// presenting more than one method in a single request. Rejected
    /// unconditionally in `handlers/oidc.rs`, before either grant is
    /// reached, so core sees this variant only in the sense that the type
    /// must be able to represent it to be checked at all.
    Both,
}

pub struct CodeExchangeRequest {
    pub code: String,
    pub redirect_uri: String,
    pub client_id: ClientId,
    /// Provided by confidential clients; None for public.
    pub client_secret: Option<String>,
    /// See [`CredentialChannel`]'s own doc comment for which fact lives
    /// here and which lives in `client_secret`.
    pub client_secret_channel: CredentialChannel,
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
    /// See [`CredentialChannel`]'s own doc comment for which fact lives
    /// here and which lives in `client_secret`.
    pub client_secret_channel: CredentialChannel,
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
    authenticate_client(
        &client,
        req.client_secret.as_deref(),
        req.client_secret_channel,
    )
    .await?;

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
fn invalid_grant() -> CoreError {
    CoreError::Protocol {
        code: ProtocolError::InvalidGrant,
        description: "refresh token is unknown or revoked".into(),
    }
}

/// Exchange a presented refresh token for a fresh token set, rotating it.
///
/// RFC 094 T04: the guarded old-row revoke (or the family-revoke it falls
/// into on reuse), the precomputed successor's insertion, and the
/// `auth.refresh.rotated`/`auth.refresh.theft_detected` event share one
/// transaction — see `sui_id_store::commands::rotate_refresh_token`'s own
/// doc comment for why that wasn't true before this conversion.
///
/// Sequence, per RFC 094's "Refresh issuance and rotation boundary": a
/// query-only preliminary read (immutable fields only — `user_id`/
/// `client_id`/`scope`/`auth_methods`/`family_id` never change after a
/// row is inserted, so reading them before the transaction is safe;
/// `revoked_at`/expiry are re-checked authoritatively inside T04), then
/// every fallible non-database step — JWT issuance, successor generation/
/// hash/seal — prepared before T04's transaction opens.
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
    authenticate_client(
        &client,
        req.client_secret.as_deref(),
        req.client_secret_channel,
    )
    .await?;

    // ── Preliminary, read-only lookup (RFC 094 T04 step 0) ───────────────
    let preliminary = match refresh_tokens::find_any(db, &req.refresh_token).await {
        Ok(row) => row,
        Err(sui_id_store::StoreError::NotFound) => return Err(invalid_grant()),
        Err(e) => return Err(e.into()),
    };
    // Fast-fail before any JWT/crypto work for an obviously wrong client;
    // T04 re-checks this authoritatively regardless (guards against a
    // race between this read and the transaction, not relied on for
    // correctness by itself).
    if preliminary.client_id != req.client_id {
        return Err(invalid_grant());
    }

    // ── RFC 095 M3 stage 4b §6: grant and scope policy ───────────────────
    // Two independent checks, not one. Both are exempt for a client whose
    // relevant column is unrecorded -- the same C1 shape as the auth-method
    // check in `authenticate_client`, applied here because `grant_types`
    // is exactly as unpersisted-until-4a as `token_endpoint_auth_method`
    // was.
    //
    // Grant policy: `refresh_token` must be among the client's stored
    // `grant_types`. Never checked before this stage -- a client that
    // never registered for refresh could always exchange one anyway.
    if let Some(grants) = &client.grant_types
        && !grants.iter().any(|g| g == "refresh_token")
    {
        return Err(CoreError::Protocol {
            code: ProtocolError::UnauthorizedClient,
            description: format!(
                "client {:?} is not registered for the refresh_token grant \
                 (registered grant_types: {grants:?})",
                client.name
            ),
        });
    }
    // Scope policy: re-checked against the client's *current*
    // `allowed_scopes`, not the scope stamped onto the refresh-token row
    // at issuance (`preliminary.scope`). Before this stage, narrowing a
    // client's scope policy after tokens were already issued had no
    // effect on those tokens' own refresh exchanges -- this is an
    // operator-visible behaviour change (CHANGELOG), not a bug fix nobody
    // will notice.
    enforce_scope_policy(&client.allowed_scopes, &preliminary.scope, &client.name)?;

    // ── Fetch user email for ID token claims (scope "email") ─────────────
    // Refresh token exchanges may include an ID token (OIDC Core §12.2).
    let email_for_token: Option<(String, bool)> =
        if preliminary.scope.split_whitespace().any(|s| s == "email") {
            match users::get(db, preliminary.user_id).await {
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

    // ── Prepare every fallible non-database input ────────────────────────
    let (access_token, id_token, access_expires_in) = issue_jwt_pair(
        db,
        clock,
        ctx,
        preliminary.user_id,
        preliminary.client_id,
        &preliminary.scope,
        None,
        &preliminary.auth_methods,
        email_arg,
    )
    .await?;

    let now = clock.now();
    let successor_row = RefreshTokenRow {
        id: RefreshTokenId::generate(),
        user_id: preliminary.user_id,
        client_id: preliminary.client_id,
        scope: preliminary.scope.clone(),
        expires_at: now + Duration::seconds(ctx.lifetimes.refresh_secs),
        revoked_at: None,
        created_at: now,
        auth_methods: preliminary.auth_methods.clone(),
        family_id: preliminary.family_id.clone(),
    };
    let prepared = refresh_tokens::prepare_refresh_token(db.key(), successor_row)?;

    // ── T04: guarded revoke, successor insert, event — one transaction ───
    let token_hash = RefreshTokenHash::of(&req.refresh_token);
    let audited =
        sui_id_store::commands::rotate_refresh_token(db, token_hash, req.client_id, now, prepared)
            .await
            .map_err(|e| match e {
                // Client-mismatch (a race against the preliminary check above) or
                // the row turning out expired/unknown by the time T04 re-checks —
                // both are the same opaque denial to the caller as before.
                sui_id_store::StoreError::Conflict | sui_id_store::StoreError::NotFound => {
                    invalid_grant()
                }
                other => other.into(),
            })?;

    match audited.into_inner() {
        // The family revocation and its audit row already committed
        // atomically inside T04 — nothing left to do here but deny. We do
        // not distinguish the "concurrent winner" case from a
        // "token-already-rotated replay" case — both look identical to
        // the caller (RFC 080 P5).
        sui_id_store::commands::T04Outcome::TheftDetected { .. } => Err(invalid_grant()),
        sui_id_store::commands::T04Outcome::Rotated { raw_token, .. } => Ok(TokenSet {
            access_token,
            id_token,
            refresh_token: raw_token,
            access_expires_in,
            user_id: Some(preliminary.user_id),
        }),
    }
}

/// `invalid_client` per RFC 6749 §5.2, with a description. The registered
/// method is not a secret -- the registrant received it in its own
/// registration response -- so naming it costs nothing against an
/// attacker and saves an integrator a debugging session (RFC 095 M3
/// stage 4b §7).
fn invalid_client(description: impl Into<String>) -> CoreError {
    CoreError::Protocol {
        code: ProtocolError::InvalidClient,
        description: description.into(),
    }
}

/// RFC 123 D4, now also enforced on this function: every branch below
/// that rejects before reaching a real `verify_password` call runs a
/// dummy one first (`password::DUMMY_PHC`), so the wall-clock cost of a
/// rejection is the same regardless of *why* it rejected. Copied from
/// `oauth_token::authenticate_client` (`:275-316`), the function this one
/// shares no code with but must match the timing shape of -- see that
/// function's own doc comment for the measurement this one's `CHANGELOG`
/// entry repeats. `provided` is whatever was actually presented, or an
/// empty placeholder when nothing was -- `DUMMY_PHC` verification takes
/// the same wall-clock time regardless of the candidate string's content.
async fn dummy_reject(provided: Option<&str>, description: impl Into<String>) -> CoreError {
    let _ =
        crate::password::verify_password(provided.unwrap_or(""), crate::password::DUMMY_PHC).await;
    invalid_client(description)
}

/// RFC 095 M3 stage 4b: the channel/method check (§3), RFC 123 D4
/// compliance (§4), and row 1's correction (§2/§B) all live here, because
/// all three are the same function and the same early-return shape.
///
/// **`!confidential` is deliberately split into two branches, not one.**
/// A public client presenting no secret is the *success* path and gets no
/// dummy hash -- RFC 123 D4's rule is about rejecting branches, and a
/// dummy `verify_password` call on every public-client token exchange
/// would cost ~34ms for no security gain (there is nothing to protect:
/// the client has no secret, and there is no "wrong secret" case to make
/// indistinguishable from "right secret" when there is no right secret).
/// A public client presenting one anyway (row 1) is new behaviour this
/// stage adds, and it *is* a rejecting branch, so it gets the dummy hash
/// like every other one below.
async fn authenticate_client(
    client: &sui_id_store::models::ClientRow,
    secret: Option<&str>,
    channel: CredentialChannel,
) -> CoreResult<()> {
    if !client.confidential {
        if secret.is_some() {
            // Row 1 (RFC 095 M3 stage 4b §3): a public client has no
            // secret to check against, so a presented one authenticates
            // nobody who could not already authenticate without it --
            // low severity, a conformance gap and not a bypass (design
            // §B) -- but it must be refused, not silently ignored.
            return Err(dummy_reject(
                secret,
                "client is registered with token_endpoint_auth_method \"none\" \
                 and must not present a client_secret",
            )
            .await);
        }
        return Ok(());
    }

    let stored = match client.secret_hash.as_deref() {
        Some(h) => h,
        None => {
            return Err(
                dummy_reject(secret, "client is confidential but has no stored secret").await,
            );
        }
    };
    let provided = match secret {
        Some(s) => s,
        None => {
            return Err(dummy_reject(None, "client_secret is required").await);
        }
    };

    // RFC 095 M3 stage 4b §0.2/§3: the channel/method check. `None` (the
    // column itself, via `ClientRow.token_endpoint_auth_method`) is the
    // unrecorded class C1 (stage 4a) decided to leave permanently exempt,
    // made visible in the admin UI since that stage. Everything else here
    // is new in this stage.
    match &client.token_endpoint_auth_method {
        None => {
            // Unrecorded: exempt, by design. Falls through to the
            // unconditional `verify_password` below, same as before this
            // stage existed.
        }
        Some(sui_id_store::models::TokenEndpointAuthMethod::Unknown(raw)) => {
            // RFC 095 M3 stage 4b §0.2: present, but not a value
            // `validate_auth_method` could ever have written -- a
            // database changed from outside. Fails closed, unlike the
            // `None` case above: an unparseable value is not evidence the
            // channel was never meant to be checked, and must not be
            // treated as if it were.
            return Err(dummy_reject(
                Some(provided),
                format!(
                    "client's registered token_endpoint_auth_method \
                     (\"{raw}\") is not recognised by this build"
                ),
            )
            .await);
        }
        Some(sui_id_store::models::TokenEndpointAuthMethod::None) => {
            // A confidential row (secret_hash present, reached this far)
            // recorded as the public method "none" is an inconsistent
            // combination no validated write path produces. No channel
            // is valid for "none", so this fails closed rather than
            // picking one.
            return Err(dummy_reject(
                Some(provided),
                "client's registered token_endpoint_auth_method is \"none\", \
                 which does not accept a client_secret on any channel",
            )
            .await);
        }
        // Row 4. Two arms, not one with a nested `if`, so the "channel
        // matches" case stays an exhaustive match arm of its own rather
        // than an empty branch inside a conditional.
        Some(sui_id_store::models::TokenEndpointAuthMethod::ClientSecretBasic)
            if channel != CredentialChannel::Basic =>
        {
            return Err(dummy_reject(
                Some(provided),
                "client's registered token_endpoint_auth_method is \
                 client_secret_basic; the secret must be presented via \
                 the Authorization: Basic header",
            )
            .await);
        }
        Some(sui_id_store::models::TokenEndpointAuthMethod::ClientSecretBasic) => {}
        // Row 6. `channel == Both` cannot reach here -- rejected
        // unconditionally in `handlers/oidc.rs` before either grant is
        // called (§5) -- so `channel == Body` already implies no Basic
        // header was present; nothing further to check for that half of
        // the row.
        Some(sui_id_store::models::TokenEndpointAuthMethod::ClientSecretPost)
            if channel != CredentialChannel::Body =>
        {
            return Err(dummy_reject(
                Some(provided),
                "client's registered token_endpoint_auth_method is \
                 client_secret_post; the secret must be presented in \
                 the request body, not the Authorization header",
            )
            .await);
        }
        Some(sui_id_store::models::TokenEndpointAuthMethod::ClientSecretPost) => {}
    }

    crate::password::verify_password(provided, stored)
        .await
        .map_err(|_| invalid_client("client authentication failed"))
}

/// Sign the access/ID-token pair. Shared by initial issuance (T09) and
/// rotation (T04) — both need identically-computed JWTs; only what
/// happens to the refresh-token row differs between them. Touches no
/// database write, so unlike the refresh-token side there is nothing
/// here for RFC 094 to make atomic with anything else.
#[allow(clippy::too_many_arguments)]
async fn issue_jwt_pair(
    db: &Database,
    clock: &SharedClock,
    ctx: IssuanceContext<'_>,
    user_id: UserId,
    client_id: ClientId,
    scope: &str,
    nonce: Option<&str>,
    auth_methods: &[sui_id_shared::AuthMethod],
    user_email: Option<(&str, bool)>,
) -> CoreResult<(String, Option<String>, i64)> {
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
    // `set.refresh_token` is `issue_token_set`'s own throwaway generation
    // (RFC 072-era shape) — neither caller here uses it: initial issuance
    // and rotation each generate and persist their own, under T09/T04
    // respectively. Dropped here; it zeroizes like any other
    // `RawRefreshToken`, never having been exposed or persisted.
    Ok((set.access_token, set.id_token, set.access_expires_in))
}

/// Initial issuance (authorization-code grant): a new refresh-token row,
/// rooting a new rotation family at its own id, through T09 — RFC 094's
/// Protocol-class command for this (`sui_id_store::commands::
/// insert_initial_refresh_token`). Never touches T04 or `Audited<T>`.
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
    let (access_token, id_token, access_expires_in) = issue_jwt_pair(
        db,
        clock,
        ctx,
        user_id,
        client_id,
        scope,
        nonce,
        auth_methods,
        user_email,
    )
    .await?;

    let now = clock.now();
    let new_id = RefreshTokenId::generate();
    let family = FamilyId::root_of(&new_id);
    let row = RefreshTokenRow {
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
    let prepared = refresh_tokens::prepare_refresh_token(db.key(), row)?;
    let raw_token = prepared.raw_token.clone();
    sui_id_store::commands::insert_initial_refresh_token(db, prepared).await?;

    Ok(TokenSet {
        access_token,
        id_token,
        refresh_token: raw_token,
        access_expires_in,
        user_id: Some(user_id),
    })
}

#[cfg(test)]
#[path = "authorize/redirect_uri_tests.rs"]
mod redirect_uri_tests;

#[cfg(test)]
#[path = "authorize/redirect_uri_matches_tests.rs"]
mod redirect_uri_matches_tests;
